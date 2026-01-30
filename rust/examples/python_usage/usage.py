#!/usr/bin/env python3

from logmech import ReaderParser

a = ReaderParser()

a.add_var("number", r"[0-9]+")
a.add_var("username", r"@(?<inside>[a-z]+)(?<parts>\.[a-z]+)*")

a.compile()

text = "123 awesrgesrgesrg 6346346 @someone foo@username @someone.foo.bar.baz"
a.set_input_stream(text)

print(f"py: input [{text}]")
while True:
	event = a.next_log_event()

	if event is None:
		break

	if event.variable is not None:
		print(f"py: static text: [{event.static_text}] {event.variable.name}: [{event.variable.text}] [{event.variable.captures!r}]")
	else:
		# Last static text before eof
		print(f"py: static text: [{event.static_text}] (end)")
