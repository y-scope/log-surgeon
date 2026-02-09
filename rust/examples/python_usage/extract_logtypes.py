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

		for token in event.tokens:
			if token.rule is None:
				template += token.text
			else:
				template += f"%{token.rule}%"
				variables.append((token.text, token.captures))

		if template in log_types:
			x = log_types[template]
		else:
			x = len(log_types)
			log_types[template] = x

		logs.append((x, variables))

templates = list(log_types.items())
templates.sort(key= lambda x: x[1])

for i in range(len(templates)):
	print(f"{i}. {templates[i]}")

print(f"{len(logs)} logs, {len(log_types)} log types")
