#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <string_view>

using namespace log_surgeon;

int main() {
    Box<Schema> schema{log_surgeon_schema_new()};

    log_surgeon_schema_add_rule(schema, "hello"_rust, "abc|d(?<foo>[a-z])f"_rust);

    ParserHandle parser{schema};

    CArray<char> const input{"def foobarbaz"_rust};
    size_t pos{0};

    std::optional<EventHandle> maybe_event{parser.next_event(input, &pos)};
    assert(maybe_event.has_value());
    assert(pos == input.length);

    EventHandle event{*maybe_event};
    assert(event.log_type() == "%hello% foobarbaz");

    std::optional<Variable> maybe_var{event.get_variable(0)};
    assert(maybe_var.has_value());

    Variable const& var{*maybe_var};
    assert(var.get_name() == "hello");

    Variable::CaptureIterator begin{var.captures_begin()};

    assert(var.capture_by_id(begin[0])[0].name.as_cpp_view() == "foo");
    assert(var.capture_by_id(begin[0])[0].lexeme.as_cpp_view() == "e");

    assert(begin + 1 == var.captures_end());

    assert(!event.get_variable(1).has_value());

    printf("good!\n");

    log_surgeon_schema_drop(schema);

    return 0;
}
