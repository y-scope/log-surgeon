log-surgeon-ffi
===============
Python bindings for [log-surgeon](https://github.com/y-scope/log-surgeon),
a high-performance library for parsing unstructured logs into structured data.

This package ships with type stubs, so IDEs like PyCharm and VS Code provide
autocompletion, parameter hints, and inline documentation out of the box.

## Installation
```bash
pip install log-surgeon-ffi
```

## Quick start
```python
from log_surgeon import Parser

parser = Parser()
parser.add_variable_pattern(
    "timestamp",
    r"(?<hours>\d{2}):(?<minutes>\d{2}):(?<seconds>\d{2})",
)
parser.add_variable_pattern("level", r"(?<level>INFO|WARN|ERROR)")
parser.compile()

parser.set_input_stream("10:30:00 INFO starting up")

while True:
    event = parser.next_log_event()
    if event is None:
        break
    print(f"log type: {event.log_type}")
    print(f"message:  {event.message}")
    for var in event.variables:
        print(f"  {var.name}: {var.text}")
        for name, values in var.captures.items():
            print(f"    {name}: {values}")
```

Output:

```text
log type: %timestamp% %level% starting up
message:  10:30:00 INFO starting up
  timestamp: 10:30:00
    hours: ['10']
    minutes: ['30']
    seconds: ['00']
  level: INFO
    level: ['INFO']
```

## Build a wheel locally
```bash
maturin build --release --out dist
pip install dist/*.whl
```
