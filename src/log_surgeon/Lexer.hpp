#ifndef LOG_SURGEON_LEXER_HPP
#define LOG_SURGEON_LEXER_HPP

#include <array>
#include <bitset>
#include <cstdint>
#include <map>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/finite_automata/Register.hpp>
#include <log_surgeon/finite_automata/TagOperation.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/ParserInputBuffer.hpp>
#include <log_surgeon/Token.hpp>

namespace log_surgeon {
using finite_automata::register_id_t;
using finite_automata::tag_id_t;

using symbol_id_t = uint32_t;

template <typename TypedNfaState, typename TypedDfaState>
class Lexer {
public:
    static inline std::vector<uint32_t> const cTokenEndTypes = {(uint32_t)SymbolId::TokenEnd};
    static inline std::vector<uint32_t> const cTokenUncaughtStringTypes
            = {(uint32_t)SymbolId::TokenUncaughtString};

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
    auto add_rule(
            uint32_t const& id,
            std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> rule
    ) -> void;

    /**
     * Return regex pattern for a rule name
     * @param variable_id
     * @return finite_automata::RegexAST*
     */
    auto get_rule(uint32_t variable_id) -> finite_automata::RegexAST<TypedNfaState>*;

    /**
     * Generate DFA for lexer.
     * @throw std::invalid_argument if `m_rules` contains multipe captures with the same name.
     */
    auto generate() -> void;

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
    auto
    scan_with_wildcard(ParserInputBuffer& input_buffer, char wildcard, Token& token) -> ErrorCode;

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

    [[nodiscard]] auto get_dfa(
    ) const -> std::unique_ptr<finite_automata::Dfa<TypedDfaState, TypedNfaState>> const& {
        return m_dfa;
    }

    [[nodiscard]] auto get_capture_ids_for_var_id(symbol_id_t const var_id
    ) const -> std::optional<std::vector<symbol_id_t>> {
        auto const capture_ids{m_var_id_to_capture_ids.find(var_id)};
        if (m_var_id_to_capture_ids.end() == capture_ids) {
            return std::nullopt;
        }
        return capture_ids->second;
    }

    [[nodiscard]] auto get_tag_ids_for_capture_id(symbol_id_t const capture_id
    ) const -> std::optional<std::pair<tag_id_t, tag_id_t>> {
        auto const tag_ids{m_capture_id_to_tag_ids.find(capture_id)};
        if (m_capture_id_to_tag_ids.end() == tag_ids) {
            return std::nullopt;
        }
        return tag_ids->second;
    }

    [[nodiscard]] auto get_reg_for_tag_id(tag_id_t const tag_id
    ) const -> std::optional<register_id_t> {
        auto const it{m_tag_to_final_reg_id.find(tag_id)};
        if (m_tag_to_final_reg_id.end() == it) {
            return std::nullopt;
        }
        return it->second;
    }

    [[nodiscard]] auto get_registers_for_capture(symbol_id_t capture_id
    ) const -> std::optional<std::pair<register_id_t, register_id_t>> {
        auto const tag_ids{get_tag_ids_for_capture_id(capture_id)};
        if (tag_ids.has_value()) {
            auto const start_reg{get_reg_for_tag_id(tag_ids.value().first())};
            auto const end_reg{get_reg_for_tag_id(tag_ids.value().second())};
            if (start_reg.has_value() && end_reg.has_value()) {
                return std::make_pair(start_reg.value(), end_reg.value());
            }
        }
        return std::nullopt;
    }

    std::unordered_map<std::string, symbol_id_t> m_symbol_id;
    std::unordered_map<symbol_id_t, std::string> m_id_symbol;

private:
    /**
     * Get next character from the input buffer
     * @return unsigned char
     */
    auto get_next_character() -> unsigned char;

    uint32_t m_match_pos{0};
    uint32_t m_start_pos{0};
    uint32_t m_match_line{0};
    uint32_t m_last_match_pos{0};
    uint32_t m_last_match_line{0};
    bool m_match{false};
    std::vector<uint32_t> const* m_type_ids{nullptr};
    std::set<uint32_t> m_type_ids_set;
    std::array<bool, cSizeOfByte> m_is_delimiter{false};
    std::array<bool, cSizeOfByte> m_is_first_char{false};
    std::vector<LexicalRule<TypedNfaState>> m_rules;
    uint32_t m_line{0};
    bool m_has_delimiters{false};
    std::unique_ptr<finite_automata::Dfa<TypedDfaState, TypedNfaState>> m_dfa;
    bool m_asked_for_more_data{false};
    TypedDfaState const* m_prev_state{nullptr};
    std::unordered_map<symbol_id_t, std::vector<symbol_id_t>> m_var_id_to_capture_ids;
    std::unordered_map<symbol_id_t, std::pair<tag_id_t, tag_id_t>> m_capture_id_to_tag_ids;
    std::map<tag_id_t, register_id_t> m_tag_to_final_reg_id;
};

namespace lexers {
using ByteLexer = Lexer<finite_automata::ByteNfaState, finite_automata::ByteDfaState>;
using Utf8Lexer = Lexer<finite_automata::Utf8NfaState, finite_automata::Utf8DfaState>;
}  // namespace lexers
}  // namespace log_surgeon

#include "Lexer.tpp"

#endif  // LOG_SURGEON_LEXER_HPP
