# Modbus RTU: read-holding-registers round trip

> **Outcome tested:** A well-formed request yields the right registers; a bad one a defined fault.

**Benchmark** (`modbus_read_holding`): 88.0 ns/op

```mermaid
flowchart TD
  A[frame in] --> B{addr == mine\nor broadcast?}
  B -- no --> Z[ignore]
  B -- yes --> C{CRC ok?}
  C -- no --> Z
  C -- yes --> D{function code}
  D -- 0x03 read-holding --> E{range in bounds?}
  E -- no --> X[exception 0x02]
  E -- yes --> R[registers response + CRC]
  D -- unsupported --> X2[exception 0x01]
```
