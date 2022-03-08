//! Driver for iterating and interacting with PCI and PCIe devices.
//!
//! ## References
//!
//! [PCI on OSDev wiki][osdev pci]
//!
//! [osdev pci]: https://wiki.osdev.org/PCI

use crate::memory::r#virtual::add_identity_mapping;
use crate::object_table;
use crate::sync::SpinLock;
use acpi::{AcpiHandler, AcpiTables, PciConfigRegions};
use alloc::sync::Arc;
use pci::Pci;

mod device;
mod table;

pub use device::PciDevice;

static PCI: SpinLock<Option<Pci>> = SpinLock::new(None);

pub unsafe fn init_acpi<H>(acpi: &AcpiTables<H>)
where
	H: AcpiHandler,
{
	let pci = PciConfigRegions::new(acpi).unwrap();
	let mut avail = [0u128; 2];
	// TODO this is ridiculous. Fork the crate or implement MCFG ourselves.
	for bus in 0..=255 {
		// IDK what a segment group is
		if pci.physical_address(0, bus, 0, 0).is_some() {
			avail[usize::from(bus >> 7)] |= 1 << (bus & 0x7f);
		}
	}
	assert_eq!(avail, [u128::MAX; 2], "todo: handle PCI bus stupidity");

	let phys = pci.physical_address(0, 0, 0, 0).unwrap();
	let size = 256 * 32 * 8 * 4096;
	let virt = add_identity_mapping(phys.try_into().unwrap(), size).unwrap();

	let pci = Pci::new(virt.cast(), phys.try_into().unwrap(), size, &[]);

	*PCI.lock() = Some(pci);

	let table = Arc::new(table::PciTable) as Arc<dyn crate::object_table::Table>;
	object_table::add_table(Arc::downgrade(&table));
	let _ = Arc::into_raw(table); // Intentionally leak the table.
}