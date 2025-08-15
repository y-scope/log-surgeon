add_executable(unit-test)
target_sources(
    unit-test
    PRIVATE
        comparison_test_utils.hpp
        test-buffer-parser.cpp
        test-capture.cpp
        test-dfa.cpp
        test-expression.cpp
        test-expression-character.cpp
        test-nfa.cpp
        test-prefix-tree.cpp
        test-query-interpretation.cpp
        test-regex-ast.cpp
        test-register-handler.cpp
        test-schema.cpp
        test-static-query-token.cpp
        test-variable-query-token.cpp
        test-wildcard-expression-view.cpp
)

target_link_libraries(
    unit-test
    PRIVATE
        Catch2::Catch2WithMain
        log_surgeon::log_surgeon
)
target_compile_features(unit-test PRIVATE cxx_std_20)

catch_discover_tests(unit-test)
