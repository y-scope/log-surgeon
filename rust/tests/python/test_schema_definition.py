#!/usr/bin/env python3

import unittest
from textwrap import dedent

from log_surgeon import Parser

class TestParsingSpecDefinition(unittest.TestCase):
	def setUp(self):
		pass

	def test_basic_roundtrip(self):
		definition = dedent("""\
		int: \\d+

		word: [a-zA-Z]\\w+
		email: (?<user>[a-z0-9.]+)@(?<domain>(?<parts>\\w+\\.)+(?<tld>\\w+))

		delimiters: \\ \\r\\t\\n
		""")

		p1 = Parser.from_parsing_spec_definition(definition)

		definition1 = p1.generate_parsing_spec_definition()

		# Because the "initial" parsing spec definition isn't canonical,
		# we need to do an extra roundtrip to compare equality.
		p2 = Parser.from_parsing_spec_definition(definition1)

		definition2 = p2.generate_parsing_spec_definition()

		self.assertEqual(definition1, definition2)
