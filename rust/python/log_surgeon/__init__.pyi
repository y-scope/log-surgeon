"""Type stubs for log_surgeon."""

from typing import IO, Optional, Union, final

__all__ = ["LogEvent", "LogType", "Parser", "Variable"]

@final
class Parser:
    """High-performance log parser using DFA-based pattern matching."""

    def __new__(cls, debug: bool = False) -> Parser: ...
    def add_variable_pattern(self, name: str, pattern: str, *, priority: int = 0) -> None:
        """Add a named pattern with ``(?<capture_name>...)`` groups.

        Higher priority patterns are matched first. Default is 0.
        """
        ...
    def set_delimiters(self, delimiters: str) -> None:
        """Set token boundary characters. Raises if empty."""
        ...
    def compile(self) -> None:
        """Compile patterns into the matching engine. Must be called before parsing."""
        ...
    def set_input_stream(self, input: Union[str, bytes, IO[bytes]]) -> None:
        """Set the input to parse (string, bytes, or readable file object)."""
        ...
    def next_log_event(self) -> Optional[LogEvent]:
        """Get the next parsed event, or ``None`` at end of input."""
        ...
    def done(self) -> bool:
        """Check if all input has been consumed."""
        ...

@final
class LogEvent:
    """A parsed log event."""

    @property
    def log_type(self) -> LogType:
        """Template with ``%rule_name%`` placeholders for matched variables."""
        ...
    @property
    def variables(self) -> list[Variable]:
        """List of variables that matched in this event."""
        ...
    @property
    def message(self) -> str:
        """The original text of the log event."""
        ...
    def __str__(self) -> str: ...

@final
class LogType:
    """A log type template showing the structure of a log event."""

    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

@final
class Variable:
    """A matched variable within a log event."""

    @property
    def name(self) -> str:
        """The rule name passed to ``add_variable_pattern()``."""
        ...
    @property
    def text(self) -> str:
        """The matched text."""
        ...
    @property
    def offsets(self) -> slice:
        """Offset of text in log message."""
        ...
    @property
    def captures(self) -> dict[str, list[str]]:
        """Capture group names mapped to their matched values."""
        ...
    def __repr__(self) -> str: ...
