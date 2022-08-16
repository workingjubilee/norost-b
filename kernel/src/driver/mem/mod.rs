use crate::{
	boot,
	memory::{
		frame::{AtomicPPNBox, PPN},
		r#virtual::RWX,
		Page,
	},
	object_table::{MemoryObject, Object, Root},
};
use alloc::sync::Arc;
use core::sync::atomic::Ordering;

static TOP: AtomicPPNBox = AtomicPPNBox::new(0);

struct Mem;

impl Object for Mem {
	fn memory_object(self: Arc<Self>) -> Option<Arc<dyn MemoryObject>> {
		Some(self)
	}
}

unsafe impl MemoryObject for Mem {
	fn physical_pages(&self, f: &mut dyn FnMut(&[PPN]) -> bool) {
		let top = TOP.load(Ordering::Relaxed);
		for i in 0..top {
			if f(&[PPN(i)]) {
				break;
			}
		}
	}

	fn physical_pages_len(&self) -> usize {
		TOP.load(Ordering::Relaxed).try_into().unwrap()
	}

	fn page_permissions(&self) -> RWX {
		RWX::RW
	}
}

pub fn init(boot: &boot::Info) {
	TOP.store(
		(boot.memory_top / Page::SIZE as u64).try_into().unwrap(),
		Ordering::Relaxed,
	);
}

pub fn post_init(root: &Root) {
	let mem = Arc::new(Mem) as Arc<dyn Object>;
	root.add(*b"mem", Arc::downgrade(&mem));
	let _ = Arc::into_raw(mem);
}
