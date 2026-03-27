#include "log_surgeon/generated_bindings.hpp"
#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <string_view>

using namespace log_surgeon;

static void try_search();

int main() {
    Box<SchemaBuilder> builder{log_surgeon_schema_builder_new()};

    log_surgeon_schema_builder_add_rule_with_priority(builder, 0, "hello"_rust, "abc|d(?<foo>[a-z])f"_rust);

    ParserHandle parser{log_surgeon_schema_builder_build(builder)};

    CArray<char> const input{"def foobarbaz"_rust};
    size_t pos{0};

    std::optional<EventHandle> maybe_event{parser.next_event(input, &pos)};
    assert(maybe_event.has_value());
    assert(pos == input.length);

    EventHandle event{*maybe_event};
    // assert(event.log_type() == "%hello% foobarbaz");
    assert(event.log_type() == "d%1.1:hello.foo%f foobarbaz");

    std::optional<CCapture> maybe_capture{event.get_leaf_capture(0)};
    assert(maybe_capture.has_value());

    CCapture const& cap{*maybe_capture};
    assert(cap.variable_name.as_cpp_view() == "hello");

    assert(!event.get_leaf_capture(1).has_value());

    assert(event.get_non_leaf_capture(0).has_value());
    assert(!event.get_non_leaf_capture(1).has_value());

    try_search();

    printf("good!\n");

    return 0;
}

static void try_search() {
    Box<SchemaBuilder> builder{log_surgeon_schema_builder_new()};

    log_surgeon_schema_builder_add_rule_with_priority(builder, 0, "foo"_rust, ":::(?<bar>[a-z]+(\\.(?<baz>[0-9]+))*)"_rust);

    Box<Schema> schema{log_surgeon_schema_builder_build(builder)};

    Option<Box<SearchResult>> search{log_surgeon_search_by_named_type(schema, "foo.bar"_rust, "hello.123.456"_rust)};

    assert(search != nullptr);
    assert(log_surgeon_search_result_get_leaf_capture(search, 0).rule_id != 0);
    assert(log_surgeon_search_result_get_leaf_capture(search, 0).capture_name == "baz"_rust);
    assert(log_surgeon_search_result_get_leaf_capture(search, 0).lexeme == "123"_rust);
    assert(log_surgeon_search_result_get_leaf_capture(search, 1).rule_id != 0);
    assert(log_surgeon_search_result_get_leaf_capture(search, 1).capture_name == "baz"_rust);
    assert(log_surgeon_search_result_get_leaf_capture(search, 1).lexeme == "456"_rust);
    assert(log_surgeon_search_result_get_leaf_capture(search, 2).rule_id == 0);
}
