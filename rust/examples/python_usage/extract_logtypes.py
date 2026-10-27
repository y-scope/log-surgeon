#!/usr/bin/env python3

from schema import parser

log_file = 'cassandra-node2.log'

with open(log_file) as f:
	contents = f.read()

	parser.set_input_stream(contents)

	lines = 0
	log_types = {}
	logs = []

	while True:
		event = parser.next_log_event()

		if event is None:
			break

		template = ""
		variables = []

		template = str(event.log_type)
		if template in log_types:
			x = log_types[template]
		else:
			x = len(log_types)
			log_types[template] = x

		logs.append((x, event.variables))

templates = list(log_types.items())
templates.sort(key= lambda x: x[1])

for i in range(len(templates)):
	print(f"{i}. {templates[i][0]}")

print(f"{len(logs)} logs, {len(log_types)} log types")
