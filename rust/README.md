# log-surgeon-ffi

Python bindings for [log-surgeon](https://github.com/y-scope/log-surgeon), a
high-performance log parsing library.

## Installation

```bash
pip install log-surgeon-ffi
```

## Quick start

```python
from log_surgeon import Parser

parser = Parser()
parser.add_variable_pattern("timestamp", r"(?<hours>\d{2}):(?<minutes>\d{2}):(?<seconds>\d{2})")
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

```
log type: %timestamp% %level% starting up
message:  10:30:00 INFO starting up
  timestamp: 10:30:00
    hours: ['10']
    minutes: ['30']
    seconds: ['00']
  level: INFO
    level: ['INFO']
```

## API

### Parser

| Method | Description |
|--------|-------------|
| `Parser(debug=False)` | Create a new parser |
| `add_variable_pattern(name, pattern, *, priority=0)` | Add a named pattern with `(?<capture_name>...)` groups |
| `set_delimiters(delimiters)` | Set token boundary characters (default: space, tab, etc.) |
| `compile()` | Compile patterns into the matching engine |
| `set_input_stream(input)` | Set input (string, bytes, or file-like object) |
| `next_log_event()` | Get next parsed event, or `None` at end of input |
| `done()` | Check if all input has been consumed |

### LogEvent

| Attribute | Description |
|-----------|-------------|
| `log_type` | Template with `%rule_name%` placeholders for matched variables |
| `message` | The original text of the log event |
| `variables` | List of `Variable` objects that matched |

### Variable

| Attribute | Description |
|-----------|-------------|
| `name` | The rule name passed to `add_variable_pattern()` |
| `text` | The matched text |
| `captures` | Dict mapping capture group names to lists of matched values |

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions and local
development setup.
