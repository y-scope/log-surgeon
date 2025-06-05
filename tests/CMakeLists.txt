add_executable(unit-test)
target_sources(unit-test
    PRIVATE
    test-buffer-parser.cpp
    test-capture.cpp
    test-dfa.cpp
    test-lexer.cpp
    test-nfa.cpp
    test-prefix-tree.cpp
    test-register-handler.cpp
)
target_link_libraries(unit-test PRIVATE Catch2::Catch2WithMain log_surgeon::log_surgeon)
target_compile_features(unit-test PRIVATE cxx_std_20)

catch_discover_tests(unit-test)
