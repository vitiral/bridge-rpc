# Universal Bridged RPC

> **This library is in the design phase.**

The universal bridged call protocol is a protocol designed to allow
a system of different devices, all using different protocols, to
communicate with each other and issue commands through (optionally) guaranteed
unique remote procedural calls.

UBR has the following requirements:
- lightweight: can be run on microcontrollers with as little as
  4k memory
- simple: The entire protocol is governed by a small set of easy to
  implement rules and there are only two kinds of devices: nodes and bridges.
  Bridges are just nodes which also store a register of existing nodes. They
  require very little memory overhead and can be run on without heap
  allocations as they simply pass data along. Cached data is only kept on the
  initiator and the executor node.
- protocol agnostic: as long as the protocol is a masterless protocol,
  UBR can run on it.
- bridged: UBR enables seamless communication between different protocols. A
  node on ethernet can communicate with a node on CAN through an arbitrary
  number of bridges.
- flexibly robust: supports any range on the spectrum of robustness vs
  guaranteed performance
    - functions can be configured to never drop their return values (until the
      initiator clears them) in order guarantee crticial data
    - functions can drop their return values immediately for idempotent
      operations and guaranteed performance.
    - functions can have a user-defined events of when they drop values
      (i.e. timeout, buffer fullness, etc)

The protocol is explicitly designed to be run on microcontrollers
with less than 4k of RAM and without dynamic memory allocation.
The initial target protocols will be:
 - **ip** (tcp): this will be initial demo support
 - ZigBee
 - UART
 - CAN

The library will be split up into several crates:
- ubq-core: contains the core data types and constants.
- ubq-logic: contains logic handlers to implement the protocol.
- ubq-std: contains higher logic implemented using the std library (including
  heap memory allocation and threading).
- ubq-uc: contains higher logic implemented for microcontrollers (no heap
  memory allocation or threading)

# Node Operation
A node works by declaring functions associated with specific `fn_id`s
which have a particular type. Ideally, the library will be able to automatically
infer the types and correctly return error codes when they are violated.

The declared functions can be called by any other node on the network by
specifying the `exec_uid` of the executor and their own node_uid as
`init_uid`. The initiator node also specifies the `cx_id` and `input_data`
for the function.

An initiator makes a call to a executor using tokens `FN_CALL`, `KILL` or `FN_GET`:
- `FN_CALL` requires `ACK` to be sent immediately if `dropable=true`.
- `FN_CALL` and `FN_GET` return one of {`ERR`, `VALUE`}
    - `FN_CALL` attempts to execute an RPC on the device
    - for `FN_CALL,stream=true`, `count` determines how much data
      to request. See section "Stream Functions".
    - for `FN_GET,stream=true`, `count` determines *which* count to
      return. The returned `VALUE` will also have `stream=true`.
    - if `stream=true`, `ERR` can be returned with:
        - `stream=false`: declares that the function ITSELF failed and there
          will be no more values.
        - `stream=true`: declares that there was an error retrieving
          specific data at `count`.
- `KILL` returns `KILLED` when executed.
    - if `dropable=true` then `CLEAR` does not have to be sent: the function
      data will also be cleared. Note that this makes `KILL+dropable=true` the
      equivalent of `CLEAR` for completed functions.

When an initiator gets a response token from the executor:
- nothing is sent when an `ACK` is received. `ACK` is *only* used to help
  understand network latency.
- `CLEAR` is sent when a `VALUE` or `ERR` is received and `dropable=false`
    - if `stream=false` this signals the function as being and all data is
      cleared.
    - if `stream=true` this signals data at `count` being received. The function
      will continue (until done and all stream data is cleared) but that index
      of the stream will be cleared.
- `CLEAR` is sent when a `KILLED` is received and `dropable=false`
    - `stream` must ALWAYS equal 0 in this case.

# Bridge Operation

TODO... bridge operation

# Function Description
Functions are the way in which the user's software communicates with other
nodes.

## Drop-able Functions
If the `dropable` bit is set, both the function call and the result of the
function are considered drop-able -- meaning that if the message is lost
in the network the data will be lost. This is extremely useful for cases where
it is better to just re-request the function than worry about lost data.
Note that any function can be made drop-able, but not all functions can be
asked to cache.

Good examples of drop-able fns are:
- `FN_PING` and `FN_HEARTBEAT`: where you just want the function to return a
  value. A dropped message simply indicates network issues, which is the point
  of these functions.
- info queries (`FN_GET_FN_INFO`, `FN_GET_HW_INFO`, etc): for always getting the
  most up-to-date information.
- reactive data stream: if you have a system that is taking in streams of data
  (i.e. tempearature, light levels, distance monitoring, etc) and reacting
  to it then it shouldn't matter if a single message is dropped -- even if that
  message were retrieved there is nothing the system can do with past data. Get
  more recent data and move on.

## Stream Functions
If the `stream` bit is set (and `count` has a value), the RPC is a
"stream" type and will output `count` values. The values will also have
`stream=1` and have their `count` set to what index of the stream they are.

The `count` in the RPC controls the number of values to be received:
- If `count=0` then the number of values will be determined by the function.
- If `count=MAX_U32` then the stream will be infinite. Some functions, like
  `heartbeat` are flexible and can output any number of `count`. Almost any
  mixture of configurations is possible.
- `count>MAX_U32/2` returns `ERR_INVALID_COUNT` since the highest value of
  `count`

The period/frequency of data transmission is implementation specific for each
function. Functions like `heartbeat` might return data every second, or they
might only return data when it becomes available (i.e. for event monitoring).

## Indexed Functions
An indexed function is a function that keeps track of an internal `cx_id`.
`cx_id` starts at 1 and is incremented each time it is called. It will ONLY
execute RPC's with `cx_id` equal to it's internal one (invalid ones get
`ERR_INVALID_CX`).

The reason for this is simple: imagine you attempt to initiate an RPC to move
your robot forward 10cm. The call is received but you get no acknoledgement,
so you make the call again. Now, what happens if the executor actually gets
*both* RPCs? It would drive forward 20cm, potentially off a cliff -- not good!

By indexing the function, we can guarantee that:
- The RPC will only be executed one time, no matter how many times
  the command is received.
- If two nodes are executing RPCs on the same device, they won't step on
  eachother's toes.

Let's look at another example. You have a thermomiter which tries to set
the temperature of the thermostat to 29C, however it's message gets delayed
for 10 minutes. Two minutes later it sets it to 25C and that works. However,
8 minutes after being set to 25C it receives the message to be set to 29C --
what should it do? (obviously stay at 25C since that is the most recent
command).

> Note: If an indexed function needs to be accessed asynchronously by multiple
> devices, the `FN_STREAM_FN_EXEC_LOG` RPC can be enabled on it to stream all
> function calls to an external database. This would allow for a device to
> determine who executed what at which `cx_id`.


# Builtin Functions
Functions are defined in the form
```
DS FN_NAME {IV_NAME: ARG_TYPE, ...} -> {RV_NAME: RV_TYPE, ...} ![ERRORS]
```
where :
- `D` is the drop-able setting
    - `G` for "`dropable` can be anything"
    - `D` for "`dropable` must be 1"
- `S` is the stream setting
    - `V` for "`stream` must be 0"
    - `s` for "`stream` can be anything with any count specified"
    - `S` for "must be stream with any `count` specified."
    - `C` for "must be stream with exact `count` specified"
    - `U` for "must be stream with `count=0` (count unknown)"
    - `I` for "must be stream with `count=INFINITE` or `count=0` specified"
- `FN_NAME` is the name of the function
- `IV` is "input value"
- `RV` is "return value"
- `ERRORS` what function specific errors can be returned (empty if none)

## Network Functions
- `FN_PING DV {} -> {} ![]` the node is expected to only return an empty value
- `FN_STREAM_HEARTBEAT DS {period_ms: int} {} ![]` return an empty value every
  `period_ms`

## Bridge Functions
These functions are how communication happens from node -> bridge and
bridge -> node. These are used in node registration and discovery.

> All of these can return bridge related errors documented in "Bridge Errors".
> These errors are represented by `BERR` below.

- `FN_REGISTER_NODE DV {} -> {} ![BERR]`: register this node with the bridge
- `FN_REGISTER_BRIDGE DV {} -> {}`: register this bridge with the node. Used
  during bridge's discovery phase.
- `FN_UNREGISTER_BRIDGE DV {} -> {}`: unregister this bridge with the node. Used
  if a bridge is stopped.
- `FN_NODE_EXISTS DV {node_id: u16} -> {exists: bool} ![BERR]`: return true if the
  bridge has the `node_id` stored (and therefore knows how to reach it).
- `FN_GET_NODES GU {} -> {node_id: [u16; 32], len: u8} ![BERR]`: return a stream
  of all nodes currently registered on this bridge in chunks of up to 32
  node_ids per value
- `FN_FORCE_DISCOVERY DV {} -> {} ![BERR]`: force the bridge to re-enter the discovery
  phase (will momentarily bring it down).
- `FN_STOP_BRIDGE DV {} -> {} ![BERR]`: force a bridge to stop acting as a
  bridge. Can be used if the bridge is misbehaving or there are too many
  bridges. Bridge will un-register with nodes before shutting down.
- `FN_START_BRIDGE DV {} -> {} ![BERR]`: tell a bridge node that has been
  stopped to be a bridge again. NOP if node is already a bridge.

## Functions about Functions
```
FN_GET_FN_INFO DV {fn_id: u32, cx_id: u32}`
    -> {running: bool, cached: bool, count: u32}`
    ![ERR_FN_ID_DNE, ERR_CX_ID_DNE]`:
```
Return information about a function with a specific `fn_id` and `cx_id`.
Since `ACK` messages are allowed to be dropped, this can be used to check if a
RPC has succeeded if no `ACK` has arrived.
- `running`: true if the function is currently running.
- `cached`: a value is available. Both `running` and `cached` can be true
  for stream functions.
- `count`: the current `count` of the function if it is stream type. Else
  `MAX_U32`.

```
FN_GET_FN_CX_IDS DU {}
    -> {fns: [{init_uid: u16, cx_id: u32}; 16], len: u8}
```
Get a stream (in batches of up to 16) of all `(init_uid, cx_id)` pairs running a
device's `fn_id`. This tells you how many of a specific function are running
concurrently.

## Node Control Functions
```
FN_GET_NODE_INFO DV {} -> {
    bridge: bool,           # true if node is a bridge
    num_networks: u16,
    max_buffer_size: u32,   # max buffer size in bytes
} ![]
```
Return info related to this library about the node.

```
FN_GET_HW_INFO DV {} -> {
    sleep: bool,        # true if device can sleep
    concurrency: bool,  # true if concurrency is supported
    ip_bus: bool,       # true if device uses IP
    can_bus: bool,      # true if device uses CAN bus
    uart_bus: bool,     # true if device uses UART
    zigbee_bus: bool,   # true if device uses ZigBee
    other_bus: bool,    # true if device has other bus

    cpu_name: [u8; 12],
    num_cores: u32,
    frequency_mhz: u32,
    ram_mb: u32,
    num_devices: u32,
  } ![]
```
Return information about the hardware of the device

```
FN_GET_RPC_INFO DV {} -> {
    rpcs_running: u32,  # how many rpcs are being run on this node
    rpcs_cached: u32,   # number of completed cached rpcs
    bytes_cached: u64,  # number of completed cached bytes
    rpcs_run: u64,      # total count of number of rpcs successfully started
} ![]
```
Return information about running processes

# The Message
The message is the data returned from all transactions in UBR. It must be able
to define the following characteristics:
- metadata about the message:
    - whether this is an error, RPC, a single-value, a stream-value, etc
        - if an error, what the error code is and what the data is.
        - if stream, what it's `count` is.
    - whether this message needs to be validated.
        - if so, what it's length and checksum are.
- context id (`cx_id`): the id that the caller has associated with this
  RPC context (call, ack, values, errors, etc).
- initiator unique id (`init_uid`): the node uid that initiated this call
- executor unique id (`exec_uid`): the node uid that executes this call
- function id (`fn_id`): the id of the function being called on the node.
    - whether this is an `sync` or `async` function call does NOT need to
      be specified in the message. If it is sync, then `index == cx_id`.
    - Note that `fn_id`'s can mean different things on different nodes.

Byte-by-Byte breakdown:
- u32: `cx_id`: RPC context id
- u8 : message metadata
- u8 : bit-reverse of message metadata (for validation)
- u16: `init_uid`: sender id
- u16: `exec_uid`: receiver id
- u16: `fn_id`: function id

Dependent Next Bytes (in this order)
- If `validation=true`: u16 data length
- If `stream=true`: u16 `count` of stream
    - `count && xEFFF`: the u16 value of count.
    - `count && x8000`: true if this message is the last of the stream
- If `ACK`, u8 ack type
- If `ERR`: u16 err type
- If `ERR`, `VALUE`, `FN_CALL`: the payload
- If `validation=true`: 5 byte CRC of WHOLE message

## Message Metadata
- bit 0: `validation` boolean
- bit 1: `dropable` boolean
- bit 2: `stream` boolean
- bit 3: `--- reserved ---`
- bit 4: `--- reserved ---`
- bit 5-7: type of message

### Message Types
Response Types:
- `0` `ACK`: acknowledge call `fn_id` of `cx_id` (+type)
- `1` `KILLED`: return `kill` completed on `fn_id` of `cx_id`
- `2` `VALUE`: return value from call `fn_id` of `cx_id` (+payload)
- `3` `ERR`: return error from a call `fn_id` of `cx_id` (+type&payload)

Reserved:
- `8`: `--- reserved ---`
- `9`: `--- reserved ---`
- `A`: `--- reserved ---`
- `B`: `--- reserved ---`

Request Types:
- `C` `FN_CALL`: call function `fn_id` using `cx_id` (+payload)
- `D` `KILL`: kill running function or stop stream `fn_id` of `cx_id`
- `E` `FN_GET`: get value/stream of already called function `fn_id` of `cx_id`

Finish Types:
- `F` `CLEAR`: clear cache of `fn_id` using `cx_id`

## Ack Types
Only requests are acknowledged. Response types can just be re-requested if they
are dropped. Receiving an ACK is *not necessary* (as `FN_GET_FN_INFO` can be
called for dropped `ACK`) , but not receiving the ack within the network
expected period gives a hint to the caller to re-request the data.

Some requests (i.e. `FN_GET`) are requesting the return of a value only *if it
already exists*. Therefore these do not return an ACK, they just return the
value.

- `ACK_FN`: acknowledge function call (call in progress)
- `ACK_CLEAR`: acknowledge clear command
- `ACK_KILL`: acknowledge a kill command (kill not yet done)

## Builtin Err Types
### Err Byte Availability Breakdown:
- `x0000` - `x00FF`: reserved for protocol specific errors
- `x0100` - `x04FF`: reserved for builtin function errors
- `x0400` - `x0FFF`: reserved for registered library errors
- `x1000` - `xFFFF`: open for user defined errors

### Protocol Errors:
#### Unexpected Message Errors:
Since messages can be sent twice (by design), it is possible that "unexpected"
messages were actually just an extra one sent that arrived after the `cx_id` was
cleared. These exactly match the bit pattern of their MsgType.

Response:
- `ERR_UNEXPECTED_ACK`: unexpected/extra `ack` received
- `ERR_UNEXPECTED_KILLED`: unexpected/extra `killed` received
- `ERR_UNEXPECTED_VALUE`: unexpected/extra `value` (or `stream_value`) received
- `ERR_UNEXPECTED_ERR`: unexpected/extra `err` received

Request:
- `ERR_UNEXPECTED_FN`: unexpected/extra `fn_call`, `fn_id` does not exist
- `ERR_UNEXPECTED_KILL`: unexpected/extra `kill` call, `fn_id` does not exist
- `ERR_UNEXPECTED_FN_GET`: unexpected/extra `fn_get` received

Finish:
- `ERR_UNEXPECTED_CLEAR`: unexpected/extra `CLEAR` received

#### Validation Errors
- `ERR_METADATA_CORRUPT`: the message-metadata != ~inverse-message-metadata
- `ERR_MESSAGE_CORRUPT`: `validation=1` and the message failed the CRC check

#### Bridge Protocol Errors
- `ERR_UNREGISTERED_INIT`: returned if an `init_id` in a message is not
  registered on a bridge. This represents a critical error.
- `ERR_UNREGISTERED_EXEC`: returned if an `exec_id` in a message is not
  registered on a bridge. Can happen if a bridge gets rebooted and
  hasn't gone through discovery yet.

#### Function Errors
- `ERR_INVALID_CX`: indexed function called with invalid context (data=current
  `cx_id`)

- `ERR_FN_MUST_DROP`: `dropable=0` for a function that must drop.
- `ERR_FN_CANT_STREAM`: `stream=1` on a function that does not support streaming.
- `ERR_FN_CANT_UNLIMITED_STREAM`: `stream=1,count=MAX_U32` on a function
  that does not support unlimited streaming.
- `ERR_FN_MUST_STREAM`: `stream=0` on a function that can only stream.
- `ERR_FN_GET_DROPABLE`: `FN_GET` was called with `dropable=1`. Drop-able
  functions will never be able to return cached data.

#### Kill Errors:
- `ERR_KILL_STOPPED`: kill failed, `fn_id` at `cx_id` is not running
- `ERR_KILL_BLOCKED`: kill failed, `fn_id` at `cx_id` is currently blocked
- `ERR_UNKILLABLE`: kill failed, `fn_id` at `cx_id` is not killable

#### Invalid Data Errors:
- `ERR_INVALID_VALUE`: invalid return value
- `ERR_INVALID_ERR_CODE`: invalid error code
- `ERR_INVALID_ERR_DATA`: invalid error data
- `ERR_INVALID_INPUT`: invalid fn input data
- `ERR_INVALID_COUNT`: invalid stream count (data=count)

#### Network Errors
These are network errors and signal to receivers that a network
may be down or that that the io workload may need to be
throttled. Bridges *can* return these.
- `ERR_NODE_UNREACHABLE`: cannot reach a node/bridge (data=`(bridge_id,node_uid)`).
- `ERR_NETWORK_FLOODED`: returned if network is up but too flooded for transfer.

#### Memory Errors: `1110 XXXX`
Device memory errors. User programs can return these as well.
- `ERR_STACK_OVERFLOW`: device ran out of stack memory
- `ERR_HEAP_FRAG`: device heap is too fragmented for msg
- `ERR_HEAP_FULL`: device heap is too full for msg

#### Library Errors: `1111 XXXX`
- `ERR_LIB_BUFFER_FULL`: the library buffer is full.
- `ERR_LIB_ERR`: error with this library's implementation

#### Protocol Log Errors
These errors will never actually be returned since they don't get sent
anywhere but a logging handler. However they are defined for the purpose
of logging/monitoring.

- `ERR_LOG_INVALID_VALUE_COUNT`: a value contained an invalid count (critical)

### Builtin Function Errors
These are errors that are returned by builtin functions, but users
can also use them if they make sense for their own functions.

#### Misc
- `ERR_INTERNAL_ERR`: unknown internal error
- `ERR_FN_ID_DNE`: returned from functions like `FN_GET_FN_INFO` if the `fn_id`
  doesn't exist.
- `ERR_CX_ID_DNE`: returned from functions like `FN_GET_FN_INFO` if the `cx_id`
  doesn't exist.

#### Bridge Errors
- `ERR_NODE_NOT_BRIDGE`: returned if bridge functions are called on a node
  that is not a bridge.

