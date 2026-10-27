#!/usr/bin/env python3

import io
import unittest

from log_surgeon import Parser
from log_surgeon.log_surgeon_ffi import LogSurgeonException

class TestParserInterface(unittest.TestCase):
	def make_parser(self, *rules, delimiters=" "):
		p = Parser()
		p.set_delimiters(delimiters)
		for rule in rules:
			name, pattern, priority = (rule if len(rule) == 3 else (*rule, 0))
			p.add_rule(name, pattern, priority=priority)
		p.compile()
		return p

	def read_events(self, p, text):
		p.set_input_stream(text)
		events = []
		while (event := p.next_log_event()) is not None:
			events.append(event)
		return events

	def test_parser_argument_validation(self):
		Parser()
		Parser(debug=True)
		with self.assertRaises(TypeError):
			Parser(True)

	def test_rule_validation(self):
		p = Parser()

		with self.assertRaises(ValueError):
			p.add_rule("", r"[a-z]+")

		with self.assertRaises(ValueError):
			p.add_rule("delimiters", r"[a-z]+")

		with self.assertRaises(ValueError):
			p.add_rule("bad", r"[")

	def test_delimiter_validation(self):
		p = Parser()

		with self.assertRaises(ValueError):
			p.set_delimiters("")

	def test_uncompiled_parser_operations(self):
		p = Parser()
		p.set_input_stream("abc")

		with self.assertRaises(LogSurgeonException):
			p.next_log_event()

		with self.assertRaises(ValueError):
			p.generate_parsing_spec_definition()

		with self.assertRaises(LogSurgeonException):
			p.reset()

	def test_input_stream_types(self):
		for text in ("abc 123", b"abc 123", io.StringIO("abc 123"), io.BytesIO(b"abc 123")):
			with self.subTest(text=text):
				p = self.make_parser(("word", r"[a-z]+"), ("num", r"[0-9]+"))
				events = self.read_events(p, text)

				self.assertEqual(len(events), 1)
				self.assertEqual(events[0].message, "abc 123")
				self.assertEqual([m.text for m in events[0].root_matches], ["abc", "123"])

	def test_invalid_input_streams(self):
		p = self.make_parser(("word", r"[a-z]+"))

		with self.assertRaises(LogSurgeonException):
			p.set_input_stream(123)

		with self.assertRaises(UnicodeDecodeError):
			p.set_input_stream(b"\xff")

	def test_done_and_eof(self):
		p = self.make_parser(("word", r"[a-z]+"), ("num", r"[0-9]+"))

		self.assertTrue(p.done())

		p.set_input_stream("abc 123")
		self.assertFalse(p.done())
		self.assertIsNotNone(p.next_log_event())
		self.assertTrue(p.done())
		self.assertIsNone(p.next_log_event())

	def test_reset(self):
		p = self.make_parser(("word", r"[a-z]+"), ("num", r"[0-9]+"))
		p.set_input_stream("abc 123")

		p.reset()
		event = p.next_log_event()
		self.assertIsNotNone(event)
		self.assertEqual(event.message, "abc 123")

		p.reset()
		self.assertIsNone(p.next_log_event())

	def test_parsing_spec_definition_round_trip(self):
		p = self.make_parser(("word", r"[a-z]+"), ("num", r"[0-9]+"))

		definition = p.generate_parsing_spec_definition()
		p2 = Parser.from_parsing_spec_definition(definition)

		events = self.read_events(p, "abc 123")
		events2 = self.read_events(p2, "abc 123")

		self.assertEqual([e.message for e in events], [e.message for e in events2])
		self.assertEqual(
			[[m.text for m in e.all_matches] for e in events],
			[[m.text for m in e.all_matches] for e in events2],
		)

	def test_invalid_parsing_spec_definition(self):
		with self.assertRaises(ValueError):
			Parser.from_parsing_spec_definition("not a parsing spec definition")

	def test_event_match_structure(self):
		p = self.make_parser(("wordint", r":(?<word>[a-z]+(?<int>[0-9]+))"))
		events = self.read_events(p, ":abc123\n")

		self.assertEqual(len(events), 1)
		event = events[0]

		self.assertEqual(event.message, ":abc123\n")
		self.assertEqual(str(event), event.message)

		self.assertEqual(len(event.root_matches), 1)
		self.assertEqual(len(event.non_leaf_matches), 2)
		self.assertEqual(len(event.leaf_matches), 1)
		self.assertEqual(len(event.all_matches), 3)

		for match in event.all_matches:
			self.assertIn(match, event.all_matches)
			self.assertIsInstance(match.root_rule_id, int)
			self.assertIsInstance(match.sub_rule_id, int)

		root = event.root_matches[0]
		word = event.non_leaf_matches[1]
		num = event.leaf_matches[0]

		self.assertFalse(root.is_leaf)
		self.assertFalse(word.is_leaf)
		self.assertTrue(num.is_leaf)

		self.assertIsNone(root.parent)
		self.assertIs(word.parent, root)
		self.assertIs(num.parent, word)

		self.assertIs(root.root, root)
		self.assertIs(word.root, root)
		self.assertIs(num.root, root)

		self.assertEqual(root.text, ":abc123")
		self.assertEqual(word.text, "abc123")
		self.assertEqual(num.text, "123")

		self.assertEqual(root.name, "wordint")
		self.assertEqual(word.name, "word")
		self.assertEqual(num.name, "int")

		self.assertEqual(root.fully_qualified_name, "wordint")
		self.assertEqual(word.fully_qualified_name, "wordint.word")
		self.assertEqual(num.fully_qualified_name, "wordint.word.int")

		self.assertEqual(root.sub_rule_id, 0)
		self.assertGreater(word.sub_rule_id, 0)
		self.assertGreater(num.sub_rule_id, 0)

		self.assertFalse(hasattr(num, "offsets"))

	def test_string_memoization(self):
		p = self.make_parser(("word", r"[a-z]+"), ("num", r"[0-9]+"))
		event = self.read_events(p, "abc 123")[0]
		match = event.leaf_matches[0]

		self.assertIs(event.message, event.message)
		self.assertIs(str(event), event.message)
		self.assertIs(match.text, match.text)

	def test_non_ascii_text(self):
		p = self.make_parser(("word", r"[a-z]+"), ("num", r"[0-9]+"))
		# Use escapes to keep this file ASCII-only.
		event = self.read_events(p, "\u00e9 234 \u4e2d\U0001f600 789")[0]

		self.assertEqual(event.message, "\u00e9 234 \u4e2d\U0001f600 789")
		self.assertEqual([m.text for m in event.leaf_matches], ["234", "789"])

if __name__ == "__main__":
	unittest.main()
