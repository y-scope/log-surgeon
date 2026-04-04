#include "log_surgeon/generated_bindings.hpp"
#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <span>
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

    std::optional<Capture> maybe_capture{event.get_leaf_capture(0)};
    assert(maybe_capture.has_value());

    Capture const& cap{*maybe_capture};
    assert(cap.ffi_pointers.variable_name.as_cpp_view() == "hello");

    assert(!event.get_leaf_capture(1).has_value());

    assert(event.get_all_captures().size() == 2);

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

    size_t len{0};
    Capture const* captures{log_surgeon_search_result_get_leaf_captures(search, &len)};
    assert(len == 2);

    assert(captures[0].rule_idx.index != 0);
    assert(captures[0].ffi_pointers.capture_name.as_cpp_view() == "baz");
    assert(captures[0].ffi_pointers.lexeme.as_cpp_view() == "123");

    assert(captures[1].rule_idx.index != 0);
    assert(captures[1].ffi_pointers.capture_name.as_cpp_view() == "baz");
    assert(captures[1].ffi_pointers.lexeme.as_cpp_view() == "456");
}
