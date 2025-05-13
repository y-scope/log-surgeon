set(
        SOURCES_LOG_SURGEON
        ../src/log_surgeon/FileReader.cpp
        ../src/log_surgeon/FileReader.hpp
        ../src/log_surgeon/finite_automata/Capture.hpp
        ../src/log_surgeon/finite_automata/DeterminizationConfiguration.hpp
        ../src/log_surgeon/finite_automata/Dfa.hpp
        ../src/log_surgeon/finite_automata/DfaState.hpp
        ../src/log_surgeon/finite_automata/DfaTransition.hpp
        ../src/log_surgeon/finite_automata/Nfa.hpp
        ../src/log_surgeon/finite_automata/NfaSpontaneousTransition.hpp
        ../src/log_surgeon/finite_automata/NfaState.hpp
        ../src/log_surgeon/finite_automata/PrefixTree.cpp
        ../src/log_surgeon/finite_automata/PrefixTree.hpp
        ../src/log_surgeon/finite_automata/RegexAST.hpp
        ../src/log_surgeon/finite_automata/RegisterHandler.hpp
        ../src/log_surgeon/finite_automata/RegisterOperation.hpp
        ../src/log_surgeon/finite_automata/StateType.hpp
        ../src/log_surgeon/finite_automata/TagOperation.hpp
        ../src/log_surgeon/Lalr1Parser.hpp
        ../src/log_surgeon/Lalr1Parser.tpp
        ../src/log_surgeon/ParserAst.cpp
        ../src/log_surgeon/ParserAst.hpp
        ../src/log_surgeon/parser_types.cpp
        ../src/log_surgeon/parser_types.hpp
        ../src/log_surgeon/ParserInputBuffer.hpp
        ../src/log_surgeon/ParserInputBuffer.cpp
        ../src/log_surgeon/Schema.hpp
        ../src/log_surgeon/Schema.cpp
        ../src/log_surgeon/SchemaParser.cpp
        ../src/log_surgeon/SchemaParser.hpp
        ../src/log_surgeon/Token.cpp
        ../src/log_surgeon/Token.hpp
        ../src/log_surgeon/types.hpp
        ../src/log_surgeon/UniqueIdGenerator.hpp
)

set(
        SOURCES_TESTS
        test-capture.cpp
        test-dfa.cpp
        test-nfa.cpp
        test-parser.cpp
        test-prefix-tree.cpp
        test-regex-ast.cpp
        test-register-handler.cpp
)

add_executable(unit-test ${SOURCES_LOG_SURGEON} ${SOURCES_TESTS})
target_link_libraries(unit-test PRIVATE Catch2::Catch2WithMain log_surgeon::log_surgeon)
target_include_directories(unit-test PRIVATE ${CMAKE_SOURCE_DIR}/src)
target_compile_features(unit-test PRIVATE cxx_std_20)

catch_discover_tests(unit-test)
