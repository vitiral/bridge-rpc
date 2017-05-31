# Bridge: A Universal Bridged RPC Protocol

> **This library is in the design phase.**

Bridge is a protocol designed to allow a system of different devices, to
communicate with each other and issue commands through (optionally) guaranteed
unique remote procedural calls. It allows inter-network communication through the
concept of "bridges". Both nodes and bridges can be highly resource constrained
devices (such as microcontrollers). Intended supported networks include tcp/ip,
UART, CAN and ZigBee.

The Bridge Protocol has the following requirements:
- lightweight: both nodes and bridges can be run on microcontrollers with as
  little as 4k RAM and no memory allocation.
- simple: The entire protocol is governed by a small set of easy to
  implement rules and there are only two kinds of devices: nodes and bridges.
  Bridges are just nodes which also store a register of existing nodes and pass
  data along to its destination.
- network agnostic: can be run on any network that is masterless and has the
  ability to broadcast (tcp/ip, UART, CAN, etc).
- bridged: enables seamless communication between different networks via
  what are known as "bridge nodes". Through bridges, a node on ethernet can
  communicate with a node on CAN (or any other protocol) through an arbitrary
  number of bridges.
- flexibly robust: supports any range on the spectrum of robustness vs
  guaranteed performance. When configured, data can always be re-requested
  and the user is guaranteed to never accidentally get duplicate data or call
  the same RPC twice.
    - functions can be configured to never drop their return values (until the
      initiator clears them) in order guarantee crticial data
    - functions can drop their return values immediately for idempotent
      operations and guaranteed performance.
    - functions can have user-defined events of when they drop values
      (i.e. timeout, buffer fullness, etc)
    - functions can store an index and only run when the index matches the
      `cx_id` given in the RPC, guaranteing that functions cannot be run
      twice accidentally.

The library will be split up into several crates:
- bridge-constants: provides generated constants for multiple languages
- bridge-logic-rs: contains core rust data types and logic handlers to implement
  the protocol.
- bridge-std-rs: contains higher logic implemented using the std library (including
  heap memory allocation and threading).
- bridge-uc-rs: contains higher logic implemented for microcontrollers (no heap
  memory allocation or threading)

The reference and primary implementation for all device types shall be in rust.

[**See the Design Documents rendered with artifact**][1]

[1]: https://vitiral.github.io/bridge-rpc/#artifacts/REQ-0
