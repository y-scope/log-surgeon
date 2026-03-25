#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <string_view>

using namespace log_surgeon;

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

    assert(event.get_capture(0).has_value());
    assert(event.get_capture(1).has_value());
    assert(!event.get_capture(2).has_value());


    printf("good!\n");

    return 0;
}
