find_package(Catch2 3 REQUIRED)

set(SOURCES_LOG_SURGEON
    src/log_surgeon/FileReader.cpp
    src/log_surgeon/FileReader.hpp
    src/log_surgeon/finite_automata/RegexAST.hpp
    src/log_surgeon/finite_automata/RegexAST.tpp
    src/log_surgeon/finite_automata/RegexNFA.hpp
    src/log_surgeon/finite_automata/RegexNFA.tpp
    src/log_surgeon/LALR1Parser.cpp
    src/log_surgeon/LALR1Parser.hpp
    src/log_surgeon/LALR1Parser.tpp
    src/log_surgeon/ParserInputBuffer.hpp
    src/log_surgeon/ParserInputBuffer.cpp
    src/log_surgeon/Schema.hpp
    src/log_surgeon/Schema.cpp
    src/log_surgeon/SchemaParser.cpp
    src/log_surgeon/SchemaParser.hpp
    src/log_surgeon/Token.cpp
    src/log_surgeon/Token.hpp
)

set(SOURCES_TESTS
    tests/test-lexer.cpp
)

add_executable(unitTest ${SOURCES_LOG_SURGEON} ${SOURCES_TESTS})
target_link_libraries(unitTest PRIVATE Catch2::Catch2WithMain)
target_include_directories(unitTest
                           PRIVATE
                           ${CMAKE_SOURCE_DIR}/src
)
target_compile_features(unitTest
                        PRIVATE cxx_std_20
)
