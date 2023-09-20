#ifndef LOG_SURGEON_LEXER_HPP
#define LOG_SURGEON_LEXER_HPP

#include <array>
#include <bitset>
#include <cstdint>
#include <memory>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/finite_automata/RegexDFA.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/ParserInputBuffer.hpp>
#include <log_surgeon/Token.hpp>

namespace log_surgeon {
template <typename NFAStateType, typename DFAStateType>
class Lexer {
public:
    // std::vector<int> can be declared as constexpr in c++20
    static inline std::vector<int> const cTokenEndTypes = {(int)SymbolID::TokenEndID};
    static inline std::vector<int> const cTokenUncaughtStringTypes = {
            (int)SymbolID::TokenUncaughtStringID};

    /**
     * A lexical rule has a name and regex pattern
     */
    struct Rule {
        // Constructor
        Rule(uint32_t n, std::unique_ptr<finite_automata::RegexAST<NFAStateType>> r)
                : m_name(n),
                  m_regex(std::move(r)) {}

        /**
         * Adds AST representing the lexical rule to the NFA
         * @param nfa
         */
        auto add_ast(finite_automata::RegexNFA<NFAStateType>* nfa) const -> void;

        uint32_t m_name;
        std::unique_ptr<finite_automata::RegexAST<NFAStateType>> m_regex;
    };

    /**
     * Add a delimiters line from the schema to the lexer
     * @param delimiters
     */
    auto add_delimiters(std::vector<uint32_t> const& delimiters) -> void;

    /**
     * Add lexical rule to the lexer's list of rules
     * @param id
     * @param regex
     */
    auto add_rule(uint32_t const& id, std::unique_ptr<finite_automata::RegexAST<NFAStateType>> rule)
            -> void;

    /**
     * Return regex pattern for a rule name
     * @param name
     * @return finite_automata::RegexAST*
     */
    auto get_rule(uint32_t const& name) -> finite_automata::RegexAST<NFAStateType>*;

    /**
     * Generate DFA for lexer
     */
    auto generate() -> void;

    /**
     * Generate DFA for a reverse lexer matching the reverse of the words in the
     * original language
     */
    auto generate_reverse() -> void;

    /**
     * Reset the lexer to start a new lexing (reset buffers, reset vars tracking
     * positions)
     */
    auto reset() -> void;

    /**
     * Set the lexer state as if it had already read a delimiter (used for
     * treating start of file as a delimiter)
     * @param input_buffer containing the data to be lexed
     */
    auto prepend_start_of_file_char(ParserInputBuffer& input_buffer) -> void;

    /**
     * Flip lexer states to match static buffer flipping.
     * @param old_storage_size The previous buffer size used to calculate the
     * new states inside the flipped buffer.
     */
    auto flip_states(uint32_t old_storage_size) -> void;

    /**
     * Gets next token from the input string
     * If next token is an uncaught string, the next variable token is already
     * prepped to be returned on the next call
     * @param input_buffer
     * @param Token&
     * @return ErrorCode::Success
     * @return ErrorCode::BufferOutOfBounds if end of input reached before
     * lexing a token.
     * @throw runtime_error("Input buffer about to overflow")
     */
    auto scan(ParserInputBuffer& input_buffer, Token& token) -> ErrorCode;

    /**
     * scan(), but with wild wildcards in the input string (for search)
     * @param input_buffer
     * @param wildcard
     * @return Token
     * @throw runtime_error("Input buffer about to overflow")
     */
    auto scan_with_wildcard(ParserInputBuffer& input_buffer, char wildcard, Token& token)
            -> ErrorCode;

    /**
     * Grows the capacity of the passed in input buffer if it is not large
     * enough to store the contents of an entire LogEvent. Then, adjusts any
     * values being tracked in the lexer related to the input buffer if needed.
     * @param parser_input_buffer Buffer which size needs to be checked.
     */
    auto increase_buffer_capacity(ParserInputBuffer& input_buffer) -> void;

    [[nodiscard]] auto get_has_delimiters() const -> bool const& { return m_has_delimiters; }

    [[nodiscard]] auto is_delimiter(uint8_t byte) const -> bool const& {
        return m_is_delimiter[byte];
    }

    // First character of any variable in the schema
    [[nodiscard]] auto is_first_char(uint8_t byte) const -> bool const& {
        return m_is_first_char[byte];
    }

    std::unordered_map<std::string, uint32_t> m_symbol_id;
    std::unordered_map<uint32_t, std::string> m_id_symbol;

private:
    /**
     * Get next character from the input buffer
     * @return unsigned char
     */
    auto get_next_character() -> unsigned char;

    /**
     * Return epsilon_closure over m_epsilon_transitions
     * @return
     */
    auto epsilon_closure(NFAStateType const* state_ptr) -> std::set<NFAStateType const*>;

    /**
     * Generate a DFA from the NFA
     * @param finite_automata::RegexNFA<NFAStateType> nfa
     * @return std::unique_ptr<finite_automata::RegexDFA<DFAStateType>>
     */
    auto nfa_to_dfa(finite_automata::RegexNFA<NFAStateType>& nfa)
            -> std::unique_ptr<finite_automata::RegexDFA<DFAStateType>>;
    uint32_t m_match_pos{0};
    uint32_t m_start_pos{0};
    uint32_t m_match_line{0};
    uint32_t m_last_match_pos{0};
    uint32_t m_last_match_line{0};
    bool m_match{false};
    std::vector<int> const* m_type_ids{nullptr};
    std::set<int> m_type_ids_set;
    std::array<bool, cSizeOfByte> m_is_delimiter{false};
    std::array<bool, cSizeOfByte> m_is_first_char{false};
    std::vector<Rule> m_rules;
    uint32_t m_line{0};
    bool m_has_delimiters{false};
    std::unique_ptr<finite_automata::RegexDFA<DFAStateType>> m_dfa;
    bool m_asked_for_more_data{false};
    DFAStateType* m_prev_state{nullptr};
};

namespace lexers {
    using ByteLexer = Lexer<finite_automata::RegexNFAByteState, finite_automata::RegexDFAByteState>;
    using UTF8Lexer = Lexer<finite_automata::RegexNFAUTF8State, finite_automata::RegexDFAUTF8State>;
}  // namespace lexers
}  // namespace log_surgeon

#include "Lexer.tpp"

#endif  // LOG_SURGEON_LEXER_HPP
