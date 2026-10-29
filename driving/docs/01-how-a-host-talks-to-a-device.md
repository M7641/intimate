# How a host talks to a USB device

Before writing a driver, settle what a driver is. Strip away every piece of
software on the PC except the USB host controller. What is left is a cable, a
fixed conversation protocol, and bytes. A driver is the code that holds up the
host's half of that conversation and turns the bytes into meaning.

This note walks the conversation from the wire up. It uses three devices
plugged into this machine:

| Device | VID:PID | Kind |
|---|---|---|
| Razer Ornata V2 keyboard | `1532:025d` | standard HID keyboard |
| Razer DeathAdder Elite mouse | `1532:005c` | standard HID mouse |
| Thrustmaster TS-XW wheel | `044f:b691` | vendor-specific, Xbox GIP protocol |

The keyboard and mouse are the reference. They work with no vendor software.
The wheel is the target. It does not, and the reason is the point of this note.

## 1. The host always speaks first

USB is host-driven. A device never sends data on its own. The host polls it,
and the device answers or says "nothing new".

- A keyboard key press waits in the keyboard until the next poll.
- The poll rate is fixed by the device. The keyboard and mouse ask for 1 ms.
  The wheel asks for 4 ms (250 Hz).

Everything below is a consequence of this rule. A driver is a poller and a
parser.

## 2. Enumeration: the device describes itself

On plug-in, the host knows nothing about the device. It runs a fixed sequence
called enumeration, over a channel every device must provide: endpoint 0, the
control endpoint.

1. Reset the device.
2. Assign it an address on the bus.
3. Ask for its descriptors.
4. Pick a configuration and switch it on.

The descriptors are small binary records. They nest:

```
Device descriptor          vendor ID, product ID, USB version
└── Configuration          power draw (wheel: 100 mA)
    └── Interface          class / subclass / protocol  <- what kind of thing
        ├── Endpoint IN    device -> host, type, size, poll interval
        └── Endpoint OUT   host -> device
```

No driver is involved yet. The USB core in the kernel does this for every
device. Linux shows the result in `/sys/bus/usb/devices/`.

## 3. The interface class decides who drives it

The interface descriptor carries three numbers: class, subclass, protocol.
The operating system matches drivers against them.

| Device | Class | Subclass | Protocol | Meaning |
|---|---|---|---|---|
| Keyboard | `03` | `01` | `01` | HID, boot-capable, keyboard |
| Mouse | `03` | `01` | `02` | HID, boot-capable, mouse |
| Wheel | `ff` | `47` | `d0` | vendor-specific |

Class `03` is HID, Human Interface Device. It is a published standard. Any OS
ships one generic HID driver, and that driver handles every keyboard, mouse,
and most joysticks. This is why the keyboard needs no software.

Class `ff` means "vendor-specific: the standard says nothing about these
bytes". No generic driver applies. Something must know the vendor's protocol.

On this machine, Linux binds the `xpad` driver to the wheel. `xpad` matches
the triple `ff/47/d0`, which Microsoft uses for Xbox One accessories. That
protocol is called GIP, Gaming Input Protocol. `xpad` treats the wheel as a
gamepad, which is why the kernel names it "Generic X-Box pad".

## 4. Endpoints and transfers

An endpoint is a one-way pipe. USB defines four transfer types:

| Type | Use |
|---|---|
| Control | Setup and commands. Endpoint 0. |
| Interrupt | Small, regular, latency-bound data. Input devices. |
| Bulk | Large data, no timing promise. Storage. |
| Isochronous | Fixed-rate streams, no retry. Audio, video. |

All three devices use interrupt endpoints:

| Device | Endpoint | Direction | Max packet | Interval |
|---|---|---|---|---|
| Keyboard | `0x81` | IN | 8 bytes | 1 ms |
| Mouse | `0x81` | IN | 8 bytes | 1 ms |
| Wheel | `0x81` | IN | 64 bytes | 4 ms |
| Wheel | `0x01` | OUT | 64 bytes | 4 ms |

The `0x80` bit marks direction: set means IN, toward the host.

The wheel has an OUT endpoint. That is the channel for commands to the wheel,
force feedback included. The keyboard sends its LED state through control
transfers instead, so it needs no OUT endpoint.

## 5. HID: the device ships its own parser

A HID device carries one more descriptor: the report descriptor. It is a
small program that describes the layout of every packet the device will send.
The generic driver reads it once, then decodes packets with no device-specific
code.

The keyboard's report descriptor, read from sysfs:

```
05 01 09 06 a1 01 05 07 19 e0 29 e7 15 00 25 01
75 01 95 08 81 02 81 01 19 00 2a ff 00 15 00 26
ff 00 75 08 95 06 81 00 05 08 19 01 29 03 15 00
25 01 75 01 95 03 91 02 95 05 91 01 c0
```

Decoded, a pair of bytes at a time:

```
05 01        Usage Page: Generic Desktop
09 06        Usage: Keyboard
a1 01        Collection: Application
  05 07        Usage Page: Keyboard keys
  19 e0 29 e7  Usages E0..E7 (Ctrl, Shift, Alt, GUI; left and right)
  15 00 25 01  Each value 0..1
  75 01 95 08  8 fields of 1 bit
  81 02        INPUT: one bit per modifier
  81 01        INPUT: 8 bits constant (reserved byte)
  19 00 2a ff 00  Usages 0..255
  15 00 26 ff 00  Each value 0..255
  75 08 95 06  6 fields of 8 bits
  81 00        INPUT: array of up to 6 pressed key codes
  05 08        Usage Page: LEDs
  19 01 29 03  Num Lock, Caps Lock, Scroll Lock
  75 01 95 03  3 fields of 1 bit
  91 02        OUTPUT: LED bits, host -> device
  95 05 91 01  OUTPUT: 5 bits padding
c0           End Collection
```

So every keyboard packet is 8 bytes:

```
byte  0         1         2..7
      modifiers reserved  up to 6 key codes
```

Holding Left Shift and pressing A:

```
02 00 04 00 00 00 00 00
│     └─ key code 0x04 = A
└─ bit 1 = Left Shift
```

Releasing both sends all zeros. The keyboard reports state, not events. The
driver diffs consecutive packets to produce "pressed" and "released".

The mouse descriptor decodes the same way to this 8-byte layout:

```
byte  0        1..2           3       4..5       6..7
      buttons  vendor-defined wheel   X (int16)  Y (int16)
```

X and Y are relative: movement since the last poll, not a position.

## 6. Boot protocol: the minimum contract

The subclass `01` on the keyboard and mouse means "boot interface". A boot
keyboard promises the fixed 8-byte layout above, with no parser needed. BIOS
firmware uses this to read a keyboard before any OS loads.

It is the simplest possible driver: poll endpoint `0x81`, read 8 bytes, look
up byte 2 in a table.

## 7. The wheel: no descriptor, no contract

The wheel has no HID report descriptor. It sends 64-byte GIP packets whose
layout is fixed by Microsoft and Thrustmaster, not described on the wire.

What is known from the `xpad` driver source:

- Every GIP packet starts with a 4-byte header: command, flags, sequence
  number, payload length.
- On plug-in, the wheel announces itself and then waits.
- The host must send a power-on command before the wheel streams input:
  `05 20 <seq> 01 00`.
- Input arrives as command `0x20`.
- Rumble goes out as command `0x09` on the OUT endpoint.

What is not known yet, and is the real work of this project:

- The exact byte layout of the wheel's `0x20` input packets. `xpad` assumes a
  gamepad layout, so steering, pedals, and paddles may land in odd places.
- The commands for real force feedback: constant force, spring, damper. `xpad`
  only sends gamepad rumble, which is why Linux reports `FF_RUMBLE` and nothing
  stronger.
- Whether the wheel needs further init commands to enable full-range input or
  force feedback.

Without software, a host can still enumerate the wheel and read its
descriptors. It cannot get a single meaningful input byte until it speaks GIP.

## 8. Where a driver can live on Linux

A driver does not have to be kernel code. Three places fit this project:

| Layer | Tool | Role |
|---|---|---|
| Observe | `usbmon` + Wireshark | Record every packet between host and wheel |
| Talk | `libusb` (userspace) | Detach `xpad`, claim the interface, read and write endpoints |
| Expose | `uinput` | Create a virtual input device that games see as a wheel |

The userspace path is: `libusb` reads raw GIP packets, our code parses them,
`uinput` publishes clean axes and accepts force-feedback effects from games.
No kernel module, no reboot on a crash.

A kernel module is the alternative. It needs no background process and
loads automatically on plug-in, but a bug can take the machine down. It can come later, once the protocol is understood.

## Next step

Capture the wheel's traffic with `usbmon` while turning the wheel and pressing
each pedal. Map every byte of the `0x20` input packet. That map is the first
real artifact of the driver.
