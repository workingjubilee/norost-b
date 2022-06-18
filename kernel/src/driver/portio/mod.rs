use crate::{
	arch::asm::io,
	object_table::{Error, Object, Root, SeekFrom, Ticket},
};
use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicU16, Ordering};

/// # Safety
///
/// This function may only be called once at boot time
pub unsafe fn init(root: &Root) {
	let io = Arc::new(Io) as Arc<dyn Object>;
	root.add(*b"portio", Arc::downgrade(&io));
	let _ = Arc::into_raw(io);
}

struct Io;

impl Object for Io {
	fn open(self: Arc<Self>, path: &[u8]) -> Ticket<Arc<dyn Object>> {
		Ticket::new_complete(if path == b"map" {
			Ok(Arc::new(IoMap { head: 0.into() }))
		} else {
			Err(Error::InvalidData)
		})
	}
}

struct IoMap {
	head: AtomicU16,
}

impl IoMap {
	fn fetch_byte(addr: u16) -> Box<[u8]> {
		dbg!(addr as *const ());
		// SAFETY: nada *shrugs*
		[unsafe { io::inb(addr) }].into()
	}

	fn put_byte(addr: u16, data: u8) {
		dbg!(addr as *const (), data as *const ());
		// SAFETY: *shrugs again*
		unsafe { io::outb(addr, data) }
	}
}

impl Object for IoMap {
	fn seek(&self, from: SeekFrom) -> Ticket<u64> {
		let n = match from {
			SeekFrom::Start(n) => n as u16,
			SeekFrom::Current(n) => self.head.load(Ordering::Relaxed).wrapping_add(n as u16),
			SeekFrom::End(n) => (n as u16).wrapping_sub(1),
		};
		self.head.store(n, Ordering::Relaxed);
		Ticket::new_complete(Ok(n.into()))
	}

	fn read(&self, length: usize) -> Ticket<Box<[u8]>> {
		Ticket::new_complete(match length {
			1 => Ok(Self::fetch_byte(self.head.fetch_add(1, Ordering::Relaxed))),
			2 => todo!(),
			4 => todo!(),
			_ => Err(Error::InvalidData),
		})
	}

	fn peek(&self, length: usize) -> Ticket<Box<[u8]>> {
		Ticket::new_complete(match length {
			1 => Ok(Self::fetch_byte(self.head.load(Ordering::Relaxed))),
			2 => todo!(),
			4 => todo!(),
			_ => Err(Error::InvalidData),
		})
	}

	fn write(self: Arc<Self>, data: &[u8]) -> Ticket<usize> {
		match data {
			&[a] => Self::put_byte(self.head.fetch_add(1, Ordering::Relaxed), a),
			&[_, _] => todo!(),
			&[_, _, _, _] => todo!(),
			_ => return Ticket::new_complete(Err(Error::InvalidData)),
		}
		Ticket::new_complete(Ok(data.len()))
	}
}
