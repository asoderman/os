use x86_64::{
    structures::paging::{page_table::PageTableEntry, PageTable, PhysFrame},
    VirtAddr,
};

struct Unused<'a>(&'a mut PageTableEntry, usize);

pub enum PageLevel<'a> {
    L4(&'a mut PageTableEntry),
    L3(&'a mut PageTableEntry),
    L2(&'a mut PageTableEntry),
    L1(&'a mut PageTableEntry),
    Phys(PhysFrame),
}

impl<'a> PageLevel<'a> {
    fn from_val(v: usize, pte: &'a mut PageTableEntry) -> Self {
        match v {
            4 => Self::L4(pte),
            3 => Self::L3(pte),
            2 => Self::L2(pte),
            1 => Self::L1(pte),
            _ => Self::L1(pte), // TODO INCORRECT
        }
    }
}

/*

fn pt_next(addr: VirtAddr, current: usize, pt: &mut PageTable) -> (&mut PageTableEntry, usize) {
    let next = match current - 1 {
        4 => addr.p4_index(),
        3 => addr.p3_index(),
        2 => addr.p2_index(),
        1 => addr.p1_index(),
        _ => return None
    };

}
*/

pub fn pt_walk(addr: VirtAddr, pt: &mut PageTable) -> Option<PageLevel> {
    let indices = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];

    let mut current_table = pt;

    let mut result = None;

    for (i, entry) in indices[0..2].into_iter().enumerate() {
        if current_table[*entry].is_unused() {
            result = Some(PageLevel::from_val(
                indices.len() - i,
                &mut current_table[*entry],
            ));
            break;
        } else {
            if i < 3 {
                current_table = unsafe {
                    (current_table[*entry].addr().as_u64() as *mut PageTable)
                        .as_mut()
                        .unwrap()
                }
            } else {
                let frame = current_table[*entry].frame().unwrap();
                result = Some(PageLevel::Phys(frame))
            }
        }
    }

    result
}
