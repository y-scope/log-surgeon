# Parsing constraints

The current design of log-surgeon imposes certain constraints on parsing logs
using variable patterns.

## Delimiters

To improve performance, the current parsing design relies on separating
variables in log events using delimiters. The schema allows the user to specify
any set of characters to represent the delimiters, but a variable pattern
*cannot* contain these delimiter characters. The one exception to this rule is
the pattern for timestamps.

## Submatch extraction

If a log has variables that are not separated by delimiters, to capture these
variables, the schema must define a single variable pattern that combines the
patterns of the two variables. After extracting the combined version, the user
must manually separate the two variables. We will remove this limitation once
we support submatch extraction.

## Timestamps

We use timestamps to find the start of new log events, enabling support for
parsing multi-line log events (as in the motivating example). The parser
considers a log event to end when it finds a newline character followed by a
timestamp (or when it reaches the end of input). If no timestamps exist in the
log file, we treat a newline character as the ending of a log event.

This creates an issue when log events contain timestamps, but do not begin with
them. Fundamentally, this is the same problem as the submatch extraction issue.
A workaround is for the user to combine any possible preceding variable patterns
or static text into the recognized timestamp patterns. This new combined pattern
will act as the timestamp denoting the start of a log event, despite containing
more than just the timestamp. The user will then have to manually separate any
merged information from the timestamp.
