use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use core::ops::Bound::{Excluded, Unbounded};

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;

use lazy_static::lazy_static;
use log::{info, trace};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::arch::x86_64::context::enter_user;
use crate::elf::Loader;
use crate::fs::{VirtualNode, null_device, Path, rootfs};
use crate::interrupt::{enable_and_halt, disable_interrupts, without_interrupts, restore_interrupts};
use crate::mm::AddressSpace;
use crate::stack::{KernelStack, UserStack};
use crate::time::{Seconds, Time};
use crate::arch::{{Context, VirtAddr}, x86_64::apic_id, x86_64::set_tss_rsp0};

lazy_static! {
    static ref PROCESS_LIST: RwLock<ProcessList> = RwLock::new(ProcessList::new());
}

pub static PROC_SWITCH_LOCK: AtomicBool = AtomicBool::new(false);

#[thread_local]
pub static mut SWITCH_PTRS: Option<(TaskHandle, TaskHandle)> = None;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[thread_local]
pub static TICKS_ELAPSED: AtomicUsize = AtomicUsize::new(0);

pub static PANIC: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub enum SchedulerError {
    ProcExists
}

pub type TaskHandle = Arc<RwLock<Task>>;

#[thread_local]
static CURRENT_PROC: AtomicUsize = AtomicUsize::new(0);

pub struct ProcessList {
    list: BTreeMap<usize, TaskHandle>,
}

impl ProcessList {
    fn new() -> Self {
        let mut list = ProcessList {
            list: BTreeMap::new(),
        };

        // Pid 0 is the state of the core before scheduling. It should never be switched to. Only
        // to be used as a "from" context
        let pid0 = Task {
            id: 0,
            core_id: None,
            address_space: None,
            parent: None,
            kstack: None,
            user_stack: None,
            entry_point: VirtAddr::new(0),

            open_files: BTreeMap::new(),

            status: Status::NotRunnable,

            arch_context: Context::default()
        };

        list.insert(Arc::new(RwLock::new(pid0))).unwrap();

        list
    }

    /// Attempts to insert a new process into the global process list.
    ///
    /// # Returns
    /// Err if unsuccessful
    pub fn insert(&mut self, task: TaskHandle) -> Result<(), SchedulerError> {
        let id = task.read().id;
        if self.list.insert(id, task).is_none() { Ok(()) } else { 
            Err(SchedulerError::ProcExists) 
        }
    }

    /// Returns the current process
    pub fn current(&self) -> TaskHandle {
        self.get(CURRENT_PROC.load(Ordering::Acquire)).clone().unwrap()
    }

    pub fn get(&self, id: usize) -> Option<TaskHandle> {
        self.list.get(&id).map(|task| Arc::clone(task))
    }

    pub fn remove(&mut self, id: usize) -> Option<TaskHandle> {
        self.list.remove(&id)
    }

    /// Update the states of all processes in the list. Assigns a core to run on if needed
    fn update_all(&self) {
        let now = Time::now();
        for (pid, proc) in self.list.iter() {
            if proc.read().core_id.is_none() && *pid != 0 {
                proc.write().core_id = Some(apic_id() as usize)
            }

            let status = proc.read().status;

            match status {
                Status::Blocked(blocker) => {
                    match blocker {
                        Wake::Time(end) => {
                            if now >= end {
                                proc.write().status = Status::Ready;
                            }
                        },
                        //_ => todo!("Implement other forms of waking")
                    };
                },
                _ => ()
            };
        }
    }

    /// Attempts to get the next runnable process. Iterates over all other processes and queries if
    /// they are runnable.
    fn get_next(&self) -> Option<TaskHandle> {
        let current_id = CURRENT_PROC.load(Ordering::Acquire);

        let other_procs = self.list.range((Excluded(current_id), Unbounded))
            .chain(self.list.range((Unbounded, Excluded(current_id))));

        for (_pid, proc) in other_procs {
            if proc.read().runnable() {
                return Some(Arc::clone(proc));
            }
        }

        None
    }

    pub fn spawn(&mut self, f: fn()) {
        let task = Task::new(next_id(), VirtAddr::new(f as u64));
        self.insert(task).unwrap();
    }
}

fn next_id() -> usize {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Wake {
    Time(Time)
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Status {
    Dying,
    Ready,
    Running,
    Blocked(Wake),
    NotRunnable,
}

#[derive(Debug)]
pub struct Task {
    pub id: usize,
    pub parent: Option<usize>,
    pub core_id: Option<usize>,
    pub address_space: Option<Box<AddressSpace>>,
    kstack: Option<KernelStack>,

    pub user_stack: Option<UserStack>,
    pub entry_point: VirtAddr,

    pub open_files: BTreeMap<usize, VirtualNode>,

    status: Status,
    arch_context: Context,
}

impl Task {
    fn new(id: usize, entry_point: VirtAddr) -> Arc<RwLock<Self>> {
        let stack = KernelStack::new();
        let mut context = Context::default();

        context.set_rsp(stack.top());
        context.push(entry_point.as_u64() as usize);

        let address_space = Box::new(AddressSpace::new_user_from_kernel());

        context.set_cr3(address_space.phys_addr());

        let mut open_files = BTreeMap::new();

        open_files.insert(0, null_device().into());
        open_files.insert(1, null_device().into());
        open_files.insert(2, null_device().into());

        info!("New task {} created", id);

        Arc::new(RwLock::new(Task {
            id,
            core_id: None,
            parent: Some(pid()),
            address_space: Some(address_space),
            kstack: Some(stack),
            user_stack: None,

            entry_point,

            open_files,
            status: Status::Ready,
            arch_context: context
        }))
    }

    /// Adds the virtual node to the process' open file list
    pub fn add_open_file(&mut self, node: VirtualNode) -> usize {
        let fd = self.open_files.last_key_value().map(|(key, _)| *key).unwrap_or(0);
        self.open_files.insert(fd + 1, node);
        fd + 1
    }

    pub fn close_file(&mut self, fd: usize) -> Result<(), crate::fs::FsError> {
        let result = self.open_files.remove(&fd);

        if let Some(node) = result {
            Ok(node.close())
        } else {
            Err(crate::fs::FsError::BadFd)
        }
    }

    fn runnable(&self) -> bool {
        // pid 0 cannot be switched into 
        if self.id == 0 {
            false
        } else {
            let this_core = self.core_id
                .map(|c| c == apic_id() as usize)
                .unwrap_or(false);

            this_core && self.status == Status::Ready
        }
    }

    pub fn blocked(&self) -> bool {
        match self.status {
            Status::Blocked(_) => true,
            _ => false
        }
    }

    pub fn dying(&self) -> bool {
        match self.status {
            Status::Dying => true,
            _ => false
        }
    }

    pub fn sleep_for(&mut self, seconds: usize) {
        let now = Time::now();
        let end = now + Seconds(seconds);
        info!("{} sleeping until {}", self.id, end);
        assert!(end > now, "now: {:#?}\nend: {:#?}", now, end);

        self.status = Status::Blocked(Wake::Time(end));
    }

    /// Prepares the task for exec. This loads the elf file to execute, resets kernel and user
    /// stacks and clears all existing usermode mappings. Once the kernel stack is reset this
    /// method pushes `enter_user()` as a return address for when the outer `exec` function returns
    fn prepare_exec(&mut self, path: Path, args: String) {
        // TODO: early return errors
        info!("Reading '{:?}' for exec", path);
        let file = rootfs().read().get_file(&path).expect("Could not locate program image");
        let image = file.contents().unwrap();

        self.user_stack = None;
        self.arch_context = Context::default();

        // Setup a default kernel stack for the process
        self.arch_context.set_rsp(self.kstack.as_ref().expect("No Kstack set in process object").top());
        self.arch_context.push(enter_user as usize);

        self.address_space.as_mut().expect("No address space for proc").remove_all().expect("Could not clear address space");

        self.load_elf(&image).expect("Could not load image. Is it a valid elf file?");

    }

    unsafe fn switch_to(&mut self, next: &mut Task) {
        // If the current process can be run again
        if self.status == Status::Running {
            self.status = Status::Ready;
        }
        next.status = Status::Running;

        CURRENT_PROC.store(next.id, Ordering::Release);

        self.arch_context.switch(&mut next.arch_context)
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        without_interrupts(|| {
            info!("Task {}, died!", self.id);
        });
    }
}

/// Performs an exec for the current process
/// Load the program image specified by the path and switch into the new program
///
/// This function will clear the existing state of the process e.g. the stack, address space etc.
pub fn exec(path: Path, args: String) {
    let stack_pointer;
    {
        if !args.is_empty() {
            todo!("Implement exec args")
        }
        let current = process_list().current();
        let mut lock = current.write();

        lock.prepare_exec(path, args);
        set_tss_rsp0(lock.arch_context.rsp());
        stack_pointer = lock.arch_context.rsp();
    }

    // Set the stack pointer to return to the address we pushed earlier
    // Do not use locals after this!
    unsafe {
        asm!("mov rsp, {}
              ret ", in(reg) stack_pointer.as_u64());
    }
}

#[track_caller]
pub fn process_list<'l>() -> RwLockReadGuard<'l, ProcessList> {
    assert!(!crate::interrupt::interrupts_enabled());
    PROCESS_LIST.read()
}

pub fn process_list_mut<'l>() -> RwLockWriteGuard<'l, ProcessList> {
    trace!("Taking proc list write lock");
    assert!(!crate::interrupt::interrupts_enabled());
    PROCESS_LIST.write()
}

extern "C" fn load_elf() {
    crate::syscall::handlers::execv(Path::from_str("/tmp/include/test_user"), String::new()).unwrap();
}

extern "C" fn load_elf_fs_test() {
    crate::syscall::handlers::execv(Path::from_str("/tmp/include/test_fs"), String::new()).unwrap();
}

extern "C" fn load_elf_fb_test() {
    crate::syscall::handlers::execv(Path::from_str("/tmp/include/test_fb"), String::new()).unwrap();
}

extern "C" fn load_elf_exec_test() {
    crate::syscall::handlers::execv(Path::from_str("/tmp/include/test_exec"), String::new()).unwrap();
}

pub fn new_user_test() {
    let task = Task::new(next_id(), VirtAddr::new(enter_user as u64));
    task.write().arch_context.push(load_elf as usize);

    process_list_mut().insert(task).unwrap();

    let task = Task::new(next_id(), VirtAddr::new(enter_user as u64));
    task.write().arch_context.push(load_elf_fs_test as usize);

    process_list_mut().insert(task).unwrap();

    let task = Task::new(next_id(), VirtAddr::new(enter_user as u64));
    task.write().arch_context.push(load_elf_fb_test as usize);

    process_list_mut().insert(task).unwrap();

    let task = Task::new(next_id(), VirtAddr::new(enter_user as u64));
    task.write().arch_context.push(load_elf_exec_test as usize);

    process_list_mut().insert(task).unwrap();
}

pub fn pid() -> usize {
    CURRENT_PROC.load(Ordering::Acquire)
}

pub fn try_pid() -> Option<usize> {
    crate::arch::x86_64::smp::is_init().then(|| pid())
}

/// Yield on the current process. Will attempt to schedule another process
pub fn yield_time() {
    let was = disable_interrupts();
    let did_switch: bool;
    unsafe {
        // Try to find something else to run. But if we can't, just wait for a new process
        did_switch = switch_next();
    }

    if !did_switch {
        trace!("Waiting to switch");
        loop {
            let blocked = process_list().current().read().blocked();
            let dying = process_list().current().read().dying();

            // if we unblock leave
            if !blocked && !dying {
                break;
            } else {
                // Otherwise wait for a proc to switch to
                enable_and_halt();
            }
        }
        trace!("leaving yield");
    }
    restore_interrupts(was);
}

pub fn exit(_status: usize) {
    {
        let current = process_list().current();

        current.write().status = Status::Dying;

        if _status != 0 {
            log::warn!("Exit status: {}", _status);
        }
    }

    yield_time();
}

/// Tries to switch to the next runnable process.
///
/// # Returns 
/// If the process was switched
pub unsafe fn switch_next() -> bool {
    while PROC_SWITCH_LOCK.compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {}
    let current = process_list().current();

    process_list().update_all();

    let next = process_list().get_next();

    if let Some(next) = next {
        trace!("process switch {} -> {}", current.read().id, next.read().id);
        SWITCH_PTRS = Some((current.clone(), next.clone()));

        TICKS_ELAPSED.store(0, Ordering::Release);
        trace!("ticks reset");

        set_tss_rsp0(next.read().arch_context.rsp());
        trace!("tss updated");
        // Do switch
        let current_ptr = current.as_mut_ptr();
        let next_ptr = next.as_mut_ptr();
        drop(current);
        drop(next);

        current_ptr.as_mut().unwrap().switch_to(next_ptr.as_mut().unwrap());

        true
    } else {
        PROC_SWITCH_LOCK.store(false, Ordering::SeqCst);
        false
    }
}

pub extern "C" fn switch_hook(_old: &mut Context, _current: &mut Context) {
    if let Some(procs) = unsafe { SWITCH_PTRS.take() } {
        let dying = procs.0.read().dying();
        if dying {
            let old_pid = procs.0.read().id;
            trace!("Pid {} dying", old_pid);
            process_list_mut().remove(old_pid);
        }
    }
    trace!("Last context: {:#X?}", _old);
    trace!("new context: {:#X?}", _current);
    crate::proc::PROC_SWITCH_LOCK.store(false, Ordering::SeqCst);
    trace!("switch success");
}
