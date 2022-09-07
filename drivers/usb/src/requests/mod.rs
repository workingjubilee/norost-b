pub mod hid;

use {
	crate::dma::Dma,
	core::{fmt, mem, slice::ArrayChunks},
};

// https://wiki.osdev.org/USB#GET_DESCRIPTOR
#[allow(dead_code)]
const GET_STATUS: u8 = 0;
#[allow(dead_code)]
const CLEAR_FEATURE: u8 = 1;
#[allow(dead_code)]
const SET_FEATURE: u8 = 3;
#[allow(dead_code)]
const SET_ADDRESS: u8 = 5;
const GET_DESCRIPTOR: u8 = 6;
#[allow(dead_code)]
const SET_DESCRIPTOR: u8 = 7;
#[allow(dead_code)]
const GET_CONFIGURATION: u8 = 8;
const SET_CONFIGURATION: u8 = 9;
#[allow(dead_code)]
const GET_INTERFACE: u8 = 10;
#[allow(dead_code)]
const SET_INTERFACE: u8 = 11;
#[allow(dead_code)]
const SYNC_FRAME: u8 = 12;

const DESCRIPTOR_DEVICE: u8 = 1;
const DESCRIPTOR_CONFIGURATION: u8 = 2;
const DESCRIPTOR_STRING: u8 = 3;
const DESCRIPTOR_INTERFACE: u8 = 4;
const DESCRIPTOR_ENDPOINT: u8 = 5;
#[allow(dead_code)]
const DESCRIPTOR_DEVICE_QUALIFIER: u8 = 6;
#[allow(dead_code)]
const DESCRIPTOR_OTHER_SPEED_CONFIGURATION: u8 = 7;
#[allow(dead_code)]
const DESCRIPTOR_INTERFACE_POWER: u8 = 8;

#[derive(Debug)]
pub enum GetDescriptor {
	Device,
	Configuration { index: u8 },
	String { index: u8 },
}

#[derive(Debug)]
pub enum DescriptorResult<'a> {
	Device(Device),
	Configuration(Configuration),
	String(DescriptorStringIter<'a>),
	Interface(Interface),
	Endpoint(Endpoint),
	Hid(hid::descriptor::Hid),
	Unknown { ty: u8, data: &'a [u8] },
	Truncated { length: u8 },
	Invalid,
}

macro_rules! into {
	($v:ident $f:ident $t:ty) => {
		pub fn $f(self) -> Option<$t> {
			match self {
				Self::$v(v) => Some(v),
				_ => None,
			}
		}
	};
}

impl<'a> DescriptorResult<'a> {
	into!(Device into_device Device);
	into!(String into_string DescriptorStringIter<'a>);
	into!(Configuration into_configuration Configuration);
}

#[derive(Debug)]
// repr(C) so the compiler doesn't try to optimize layout and subsequently deoptimizes decode.
#[repr(C)]
pub struct Device {
	pub usb: u16,
	pub class: u8,
	pub subclass: u8,
	pub protocol: u8,
	pub max_packet_size_0: u8,
	pub vendor: u16,
	pub product: u16,
	pub device: u16,
	pub index_manufacturer: u8,
	pub index_product: u8,
	pub index_serial_number: u8,
	pub num_configurations: u8,
}

#[derive(Debug)]
// ditto
#[repr(C)]
pub struct Configuration {
	pub total_length: u16,
	pub num_interfaces: u8,
	pub configuration_value: u8,
	/// Value which when used as an argument in the SET_CONFIGURATION request,
	/// causes the device to assume the configuration described by this descriptor.
	pub index_configuration: u8,
	pub attributes: ConfigurationAttributes,
	pub max_power: u8,
}

pub struct ConfigurationAttributes(u8);

macro_rules! flag {
	($i:literal $f:ident) => {
		fn $f(&self) -> bool {
			self.0 & 1 << $i != 0
		}
	};
}

impl ConfigurationAttributes {
	flag!(6 self_powered);
	flag!(5 remote_wakeup);
}

impl fmt::Debug for ConfigurationAttributes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut f = f.debug_set();
		self.self_powered()
			.then(|| f.entry(&format_args!("SELF_POWERED")));
		self.remote_wakeup()
			.then(|| f.entry(&format_args!("REMOTE_WAKEUP")));
		f.finish()
	}
}

#[derive(Debug)]
// ditto
#[repr(C)]
pub struct Interface {
	pub number: u8,
	pub alternate_setting: u8,
	pub num_endpoints: u8,
	pub class: u8,
	pub subclass: u8,
	pub protocol: u8,
	pub index: u8,
}

#[derive(Debug)]
// ditto
#[repr(C)]
pub struct Endpoint {
	/// The address of the endpoint on the USB device described by this descriptor.
	pub address: EndpointAddress,
	pub attributes: EndpointAttributes,
	pub max_packet_size: u16,
	pub interval: u8,
}

pub struct EndpointAddress(u8);

impl EndpointAddress {
	pub fn direction(&self) -> Direction {
		if self.0 & 1 << 7 == 0 {
			Direction::Out
		} else {
			Direction::In
		}
	}

	pub fn number(&self) -> EndpointNumber {
		use EndpointNumber::*;
		match self.0 & 0xf {
			1 => N1,
			2 => N2,
			3 => N3,
			4 => N4,
			5 => N5,
			6 => N6,
			7 => N7,
			8 => N8,
			9 => N9,
			10 => N10,
			11 => N11,
			12 => N12,
			13 => N13,
			14 => N14,
			15 => N15,
			_ => unreachable!(),
		}
	}
}

impl fmt::Debug for EndpointAddress {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(stringify!(EndpointAddress))
			.field("direction", &self.direction())
			.field("number", &self.number())
			.finish()
	}
}

#[derive(Debug)]
pub enum EndpointNumber {
	N1,
	N2,
	N3,
	N4,
	N5,
	N6,
	N7,
	N8,
	N9,
	N10,
	N11,
	N12,
	N13,
	N14,
	N15,
}

impl From<EndpointNumber> for usize {
	fn from(n: EndpointNumber) -> usize {
		use EndpointNumber::*;
		match n {
			N1 => 1,
			N2 => 2,
			N3 => 3,
			N4 => 4,
			N5 => 5,
			N6 => 6,
			N7 => 7,
			N8 => 8,
			N9 => 9,
			N10 => 10,
			N11 => 11,
			N12 => 12,
			N13 => 13,
			N14 => 14,
			N15 => 15,
		}
	}
}

pub struct EndpointAttributes(u8);

impl EndpointAttributes {
	pub fn usage(&self) -> EndpointUsage {
		match self.0 >> 4 & 0x3 {
			0 => EndpointUsage::Data,
			1 => EndpointUsage::Feedback,
			2 => EndpointUsage::Implicit,
			_ => unreachable!(),
		}
	}

	pub fn sync(&self) -> EndpointSync {
		match self.0 >> 2 & 0x3 {
			0 => EndpointSync::None,
			1 => EndpointSync::Async,
			2 => EndpointSync::Adapt,
			3 => EndpointSync::Sync,
			_ => unreachable!(),
		}
	}

	pub fn transfer(&self) -> EndpointTransfer {
		match self.0 & 0x3 {
			0 => EndpointTransfer::Control,
			1 => EndpointTransfer::Isoch,
			2 => EndpointTransfer::Bulk,
			3 => EndpointTransfer::Interrupt,
			_ => unreachable!(),
		}
	}
}

impl fmt::Debug for EndpointAttributes {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct(stringify!(EndpointAttributes))
			.field("usage", &self.usage())
			.field("sync", &self.sync())
			.field("transfer", &self.transfer())
			.finish()
	}
}

#[derive(Debug)]
pub enum EndpointUsage {
	Data,
	Feedback,
	Implicit,
}

#[derive(Debug)]
pub enum EndpointSync {
	None,
	Async,
	Adapt,
	Sync,
}

#[derive(Debug)]
pub enum EndpointTransfer {
	Control,
	Isoch,
	Bulk,
	Interrupt,
}

#[derive(Debug)]
pub struct DescriptorStringIter<'a>(ArrayChunks<'a, u8, 2>);

#[derive(Debug)]
pub enum Request {
	GetDescriptor { ty: GetDescriptor, buffer: Dma<[u8]> },
	SetConfiguration { value: u8 },
	Hid(hid::report::Request),
}

pub struct RawRequest {
	pub request_type: u8,
	pub direction: Direction,
	pub request: u8,
	pub value: u16,
	pub index: u16,
	pub buffer: Option<Dma<[u8]>>,
}

mod request_type {
	pub const DIR_OUT: u8 = 0 << 7;
	pub const DIR_IN: u8 = 1 << 7;

	pub const TYPE_STANDARD: u8 = 0 << 5;
	pub const TYPE_CLASS: u8 = 1 << 5;
	pub const TYPE_VENDOR: u8 = 2 << 5;

	pub const RECIPIENT_DEVICE: u8 = 0;
	pub const RECIPIENT_INTERFACE: u8 = 1;
	pub const RECIPIENT_ENDPOINT: u8 = 2;
	pub const RECIPIENT_OTHER: u8 = 3;
}

#[derive(Debug)]
pub enum Direction {
	In,
	Out,
}

impl Request {
	pub fn into_raw(self) -> RawRequest {
		use request_type::*;
		match self {
			Self::GetDescriptor { ty, buffer } => RawRequest {
				request_type: DIR_IN | TYPE_STANDARD | RECIPIENT_DEVICE,
				direction: Direction::In,
				request: GET_DESCRIPTOR,
				value: match ty {
					GetDescriptor::Device => u16::from(DESCRIPTOR_DEVICE) << 8,
					GetDescriptor::Configuration { index } => {
						u16::from(DESCRIPTOR_CONFIGURATION) << 8 | u16::from(index)
					}
					GetDescriptor::String { index } => {
						u16::from(DESCRIPTOR_STRING) << 8 | u16::from(index)
					}
				},
				index: 0,
				buffer: Some(buffer),
			},
			Self::SetConfiguration { value } => RawRequest {
				request_type: DIR_OUT | TYPE_STANDARD | RECIPIENT_DEVICE,
				direction: Direction::Out,
				request: SET_CONFIGURATION,
				value: value.into(),
				index: 0,
				buffer: None,
			},
			Self::Hid(h) => h.into_raw(),
		}
	}
}

impl<'a> DescriptorResult<'a> {
	pub fn decode(buf: &'a [u8]) -> Self {
		decode(buf).next().unwrap_or(Self::Invalid)
	}
}

impl Iterator for DescriptorStringIter<'_> {
	type Item = u16;

	fn next(&mut self) -> Option<Self::Item> {
		self.0.next().copied().map(u16::from_le_bytes)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len(), Some(self.len()))
	}
}

impl ExactSizeIterator for DescriptorStringIter<'_> {
	fn len(&self) -> usize {
		self.0.len()
	}
}

fn decode_device(buf: &[u8]) -> Device {
	let f1 = |i: usize| buf[i - 2];
	let f2 = |i: usize| u16::from_le_bytes(buf[i - 2..i].try_into().unwrap());
	Device {
		usb: f2(2),
		class: f1(4),
		subclass: f1(5),
		protocol: f1(6),
		max_packet_size_0: f1(7),
		vendor: f2(8),
		product: f2(10),
		device: f2(12),
		index_manufacturer: f1(14),
		index_product: f1(15),
		index_serial_number: f1(16),
		num_configurations: f1(17),
	}
}

fn decode_configuration<'a>(buf: &'a [u8]) -> Configuration {
	let f1 = |i: usize| buf[i - 2];
	let f2 = |i: usize| u16::from_le_bytes(buf[i - 2..i].try_into().unwrap());
	let num_interfaces = f1(4);
	Configuration {
		total_length: f2(2),
		num_interfaces,
		configuration_value: f1(5),
		index_configuration: f1(6),
		attributes: ConfigurationAttributes(f1(7)),
		max_power: f1(8),
	}
}

fn decode_string(buf: &[u8]) -> DescriptorStringIter<'_> {
	DescriptorStringIter(buf.array_chunks::<2>())
}

fn decode_interface(buf: &[u8]) -> Interface {
	let f1 = |i: usize| buf[i - 2];
	let num_endpoints = f1(4);
	Interface {
		number: f1(2),
		alternate_setting: f1(3),
		num_endpoints,
		class: f1(5),
		subclass: f1(6),
		protocol: f1(7),
		index: f1(8),
	}
}

fn decode_endpoint(buf: &[u8]) -> Endpoint {
	let f1 = |i: usize| buf[i - 2];
	let f2 = |i: usize| u16::from_le_bytes(buf[i - 2..i].try_into().unwrap());
	Endpoint {
		address: EndpointAddress(f1(2)),
		attributes: EndpointAttributes(f1(3)),
		max_packet_size: f2(4),
		interval: f1(6),
	}
}

pub fn decode(buf: &[u8]) -> Iter<'_> {
	Iter { buf }
}

pub struct Iter<'a> {
	buf: &'a [u8],
}

impl<'a> Iterator for Iter<'a> {
	type Item = DescriptorResult<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		(!self.buf.is_empty()).then(|| {
			let buf = mem::take(&mut self.buf);
			let l = buf[0];
			if l < 2 {
				return DescriptorResult::Invalid;
			}
			if usize::from(l) > buf.len() {
				return DescriptorResult::Truncated { length: l };
			}
			let b = &buf[2..usize::from(l)];
			let r = match buf[1] {
				DESCRIPTOR_DEVICE => DescriptorResult::Device(decode_device(b)),
				DESCRIPTOR_CONFIGURATION => {
					DescriptorResult::Configuration(decode_configuration(b))
				}
				DESCRIPTOR_STRING => DescriptorResult::String(decode_string(b)),
				DESCRIPTOR_INTERFACE => DescriptorResult::Interface(decode_interface(b)),
				DESCRIPTOR_ENDPOINT => DescriptorResult::Endpoint(decode_endpoint(b)),
				hid::descriptor::HID => DescriptorResult::Hid(hid::descriptor::decode_hid(b)),
				ty => DescriptorResult::Unknown { ty, data: b },
			};
			self.buf = &buf[usize::from(l)..];
			r
		})
	}
}
