use super::{Error, Object, StreamingTableOwner, Ticket};
use crate::{object_table::QueryIter, sync::Mutex};
use alloc::{
	boxed::Box,
	collections::BTreeMap,
	sync::{Arc, Weak},
	vec::Vec,
};
use core::mem;

/// A root object. This object has multiple child objects which can be accessed by a name, e.g.
///
/// ```
/// net/
/// 	tcp
/// 	...
/// disk/
/// 	data
/// fs/
/// 	bin/
/// 	README/
/// 	...
/// process/
/// ```
pub struct Root {
	objects: Mutex<BTreeMap<Box<[u8]>, Weak<dyn Object>>>,
}

impl Root {
	/// Create a new root
	pub fn new() -> Self {
		Self {
			objects: Default::default(),
		}
	}

	/// Add a new object to the root.
	pub fn add(&self, name: impl Into<Box<[u8]>>, object: Weak<dyn Object>) {
		self.objects.lock().insert(name.into(), object);
	}

	fn find<'a>(&self, path: &'a [u8]) -> Option<(Arc<dyn Object>, &'a [u8], Option<&'a [u8]>)> {
		let (object, rest) = path
			.iter()
			.position(|c| *c == b'/')
			.map_or((path, None), |i| (&path[..i], Some(&path[i + 1..])));
		let mut objects = self.objects.lock();
		if let Some(obj) = objects.get(object) {
			if let Some(obj) = Weak::upgrade(&obj) {
				Some((obj, object, rest))
			} else {
				objects.remove(object);
				None
			}
		} else {
			None
		}
	}
}

impl Object for Root {
	fn open(self: Arc<Self>, path: &[u8]) -> Ticket<Arc<dyn Object>> {
		if path == b"" || path == b"/" {
			Ticket::new_complete(Ok(Arc::new(QueryIter::new(
				self.objects
					.lock()
					.keys()
					.map(|s| s.to_vec())
					.collect::<Vec<_>>()
					.into_iter(),
			))))
		} else {
			self.find(path)
				.map_or_else(not_found, |(obj, _, path)| match path {
					None => Ticket::new_complete(Ok(obj)),
					Some(path) => obj.open(path),
				})
		}
	}

	fn create(self: Arc<Self>, path: &[u8]) -> Ticket<Arc<dyn Object>> {
		Ticket::new_complete(if path.is_empty() {
			Err(Error::InvalidData)
		} else if let Some((obj, _, path)) = self.find(path) {
			match path {
				None => Err(Error::AlreadyExists),
				Some(path) => return obj.create(path),
			}
		} else if path.contains(&b'/') {
			Err(Error::DoesNotExist)
		} else {
			Ok(Arc::new(CreateRootEntry {
				root: self,
				name: Mutex::new(path.into()),
			}))
		})
	}
}

struct CreateRootEntry {
	root: Arc<Root>,
	name: Mutex<Box<[u8]>>,
}

impl Object for CreateRootEntry {
	fn share(&self, share: &Arc<dyn Object>) -> Ticket<u64> {
		let mut name = self.name.lock();
		Ticket::new_complete(if name.is_empty() {
			Err(Error::InvalidOperation)
		} else {
			self.root.add(mem::take(&mut *name), Arc::downgrade(share));
			Ok(0)
		})
	}
}

fn not_found<T>() -> Ticket<T> {
	Ticket::new_complete(Err(Error::DoesNotExist))
}
