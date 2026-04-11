#!/usr/bin/env python3

import unittest
from textwrap import dedent

from log_surgeon import Parser

class TestSchemaDefinition(unittest.TestCase):
	def setUp(self):
		pass

	def test_basic_roundtrip(self):
		definition = dedent("""\
		int: \\d+

		word: [a-zA-Z]\\w+
		email: (?<user>[a-z0-9.]+)@(?<domain>(?<parts>\\w+\\.)+(?<tld>\\w+))

		delimiters: \\ \\r\\t\\n
		""")

		p1 = Parser.from_schema_definition(definition)

		definition1 = p1.generate_schema_definition()

		# Because the "initial" schema definition isn't canonical,
		# we need to do an extra roundtrip to compare equality.
		p2 = Parser.from_schema_definition(definition1)

		definition2 = p2.generate_schema_definition()

		self.assertEqual(definition1, definition2)
