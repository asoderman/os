use core::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use core::ops::Bound::{Excluded, Unbounded};

use alloc::sync::Arc;
use alloc::collections::BTreeMap;

use lazy_static::lazy_static;
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::interrupt::{enable_and_halt, disable_interrupts, without_interrupts};
use crate::time::{Seconds, Time};
use crate::{arch::{Context, VirtAddr, x86_64::apic_id, x86_64::set_tss_rsp0}, stack::allocate_kernel_stack};

lazy_static! {
    static ref PROCESS_LIST: RwLock<ProcessList> = RwLock::new(ProcessList::new());
}

pub static PROC_SWITCH_LOCK: AtomicBool = AtomicBool::new(false);

#[thread_local]
pub static mut SWITCH_PTRS: Option<(TaskHandle, TaskHandle)> = None;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[thread_local]
pub static TICKS_ELAPSED: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub enum SchedulerError {
    ProcExists
}

pub type TaskHandle = Arc<RwLock<Task>>;

#[thread_local]
static CURRENT_PROC: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
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
            parent: None,
            entry_point: VirtAddr::new(0),
            kstack: VirtAddr::new(0),

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
        for (pid, proc) in self.list.iter() {
            if proc.read().core_id.is_none() && *pid != 0 {
                proc.write().core_id = Some(apic_id() as usize)
            }

            let status = proc.read().status;

            match status {
                Status::Blocked(blocker) => {
                    match blocker {
                        Wake::Time(end) => {
                            let now = Time::now();
                            if now >= end {
                                proc.write().status = Status::Ready;
                            }
                        },
                        _ => todo!("Implement other forms of waking")
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
    entry_point: VirtAddr,
    kstack: VirtAddr,
    status: Status,
    arch_context: Context,
}

impl Task {
    fn new(id: usize, entry_point: VirtAddr) -> Arc<RwLock<Self>> {
        let stack = allocate_kernel_stack();
        let mut context = Context::default();

        context.set_rsp(stack);
        context.push(entry_point.as_u64() as usize);

        context.set_cr3(crate::arch::x86_64::paging::get_cr3());

        Arc::new(RwLock::new(Task {
            id,
            core_id: None,
            parent: Some(pid()),
            entry_point,
            kstack: stack,
            status: Status::Ready,
            arch_context: context
        }))
    }

    fn runnable(&self) -> bool {
        // pid 0 cannot be switched into 
        if self.id == 0 {
            false
        } else {
            let this_core = self.core_id
                .map(|c| c == apic_id() as usize)
                .unwrap_or(true);

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
        assert!(end > now, "now: {:#?}\nend: {:#?}", now, end);

        self.status = Status::Blocked(Wake::Time(end));
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
            println!("Task {}, died!", self.id);
        });
    }
}

pub fn process_list<'l>() -> RwLockReadGuard<'l, ProcessList> {
    PROCESS_LIST.read()
}

pub fn process_list_mut<'l>() -> RwLockWriteGuard<'l, ProcessList> {
    PROCESS_LIST.write()
}

pub fn pid() -> usize {
    CURRENT_PROC.load(Ordering::Acquire)
}

/// Yield on the current process. Will attempt to schedule another process
pub fn yield_time() {
    let did_switch: bool;
    unsafe {
        // Try to find something else to run. But if we can't, just wait for a new process
        did_switch = switch_next();
    }

    if !did_switch {
        loop {
            disable_interrupts();
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
    }
}

pub fn exit(_status: usize) {
    let current = process_list().current();

    // TODO: do resource cleanup
    current.write().status = Status::Dying;

    drop(current);

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
        SWITCH_PTRS = Some((current.clone(), next.clone()));

        TICKS_ELAPSED.store(0, Ordering::SeqCst);

        set_tss_rsp0(next.read().arch_context.rsp());
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
        let old_pid = procs.0.read().id;
        let dying = procs.0.read().dying();
        if dying {
            println!("removed: {:?}", process_list_mut().remove(old_pid));
        }
    }
    crate::proc::PROC_SWITCH_LOCK.store(false, core::sync::atomic::Ordering::SeqCst);
}
