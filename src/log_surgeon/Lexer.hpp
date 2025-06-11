#ifndef LOG_SURGEON_LEXER_HPP
#define LOG_SURGEON_LEXER_HPP

#include <array>
#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/ParserInputBuffer.hpp>
#include <log_surgeon/Token.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {
/**
 * Represents a lexer that processes input buffers using a DFA-based approach.
 *
 * The `Lexer` class tokenizes input data based on lexical rules defined using regular expressions.
 * It supports adding delimiters, scanning for tokens, and handling captures for tagged expressions.
 * The lexer can also be used to lex search queries by considering `?` and `*` wildcards.
 *
 * Lexical rules are associated with rule names, which can be non-unique. When multiple rules share
 * the same rule ID, the lexer uses the union (i.e., logical OR) of these rules for tokenization.
 *
 * Lexical rules that include context introduce captures, tags, and registers. Each capture maps
 * to a start and end tag, identifying the capture's position in the token. Each tag maps to several
 * registers to track potential locations during DFA traversal.
 *
 * @tparam TypedNfaState The type representing NFA states used in rule definitions.
 * @tparam TypedDfaState The type representing DFA states used for token scanning.
 */
template <typename TypedNfaState, typename TypedDfaState>
class Lexer {
public:
    static inline std::vector<uint32_t> const cTokenEndTypes = {(uint32_t)SymbolId::TokenEnd};
    static inline std::vector<uint32_t> const cTokenUncaughtStringTypes
            = {(uint32_t)SymbolId::TokenUncaughtString};

    /**
     * Sets a list of delimiter types to the lexer, invalidating any delimiters that were previously
     * set.
     *
     * @param delimiters A non-empty vector containing the delimiter types to be added.
     */
    auto set_delimiters(std::vector<uint32_t> const& delimiters) -> void;

    /**
     * Adds a lexical rule to the lexer. If a `rule_id` is repeated, the lexer will use the union
     * (i.e., logical OR) of all the corresponding `rule`s.
     *
     * @param rule_id The identifier for the rule.
     * @param rule A unique pointer to a `RegexAST` representing the rule.
     */
    auto add_rule(rule_id_t rule_id, std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> rule)
            -> void;

    /**
     * Retrieves a regex rule for a given id.
     * @param rule_id The ID of the rule to retrieve.
     * @return A pointer to the corresponding `RegexAST` object.
     */
    [[nodiscard]] auto get_highest_priority_rule(uint32_t rule_id)
            -> finite_automata::RegexAST<TypedNfaState>*;

    /**
     * Generates the DFA for the lexer.
     * @throw std::invalid_argument If multiple captures with the same name are found in the rules.
     */
    auto generate() -> void;

    /**
     * Reset the lexer to start a new lexing (reset buffers, reset vars tracking positions).
     */
    auto reset() -> void;

    /**
     * Set the lexer state as if it had already read a delimiter (used for treating start of file as
     * a delimiter).
     * @param input_buffer Buffer containing the data to be lexed.
     */
    auto prepend_start_of_file_char(ParserInputBuffer& input_buffer) -> void;

    /**
     * Flip the lexer's states to match static buffer flipping.
     * @param old_storage_size The previous buffer size used to calculate the new states inside the
     * flipped buffer.
     */
    auto flip_states(uint32_t old_storage_size) -> void;

    /**
     * Scans the input buffer and retrieves the next token.
     * If the next token is an uncaught string, the next variable token is already prepped to be
     * returned on the next call.
     * @param input_buffer The input buffer to scan.
     * @return If lexing is completed, a pair containing:
     * - `ErrorCode::Success`.
     * - The lexed token.
     * @return If the end of the input is reached before lexing is completed, a pair containing:
     * - `ErrorCode::BufferOutOfBounds`.
     * - `std::nullopt`.
     * @throw runtime_error if the input buffer is about to overflow.
     */
    auto scan(ParserInputBuffer& input_buffer) -> std::pair<ErrorCode, std::optional<Token>>;

    /**
     * Scans the input buffer with wildcards, allowing for more flexible token matching.
     * This is useful for searching with patterns that include wildcards.
     * @param input_buffer The input buffer to scan.
     * @param wildcard The wildcard character used for matching.
     * @param token The token object to be populated with the matched token.
     * @return ErrorCode::Success if a token is successfully scanned.
     * @throw runtime_error if the input buffer is about to overflow.
     */
    auto scan_with_wildcard(ParserInputBuffer& input_buffer, char wildcard, Token& token)
            -> ErrorCode;

    /**
     * Increases the capacity of the input buffer if it is not large enough to store an entire
     * LogEvent. Adjusts the lexer's tracking of the buffer position if necessary.
     * @param input_buffer The buffer whose size needs to be checked and potentially increased.
     */
    auto increase_buffer_capacity(ParserInputBuffer& input_buffer) -> void;

    [[nodiscard]] auto get_has_delimiters() const -> bool const& { return m_has_delimiters; }

    [[nodiscard]] auto is_delimiter(uint8_t byte) const -> bool const& {
        return m_is_delimiter[byte];
    }

    [[nodiscard]] auto get_dfa() const
            -> std::unique_ptr<finite_automata::Dfa<TypedDfaState, TypedNfaState>> const& {
        return m_dfa;
    }

    /**
     * Retrieves a list of capture IDs for a given rule ID.
     * These capture IDs correspond to the captures in the rule that were matched during lexing.
     * @param rule_id The ID of the rule to search for captures.
     * @return A vector of capture IDs if the rule contains captures;
     * @return std::nullopt if no captures are found for the rule.
     */
    [[nodiscard]] auto get_capture_ids_from_rule_id(rule_id_t const rule_id) const
            -> std::optional<std::vector<capture_id_t>> {
        if (m_rule_id_to_capture_ids.contains(rule_id)) {
            return m_rule_id_to_capture_ids.at(rule_id);
        }
        return std::nullopt;
    }

    /**
     * @param capture_id ID associated with a capture within a rule.
     * @return The start and end tag of the capture on success.
     * @return std::nullopt if no capture is associated with the given capture ID.
     */
    [[nodiscard]] auto get_tag_id_pair_from_capture_id(capture_id_t const capture_id) const
            -> std::optional<std::pair<tag_id_t, tag_id_t>> {
        if (m_capture_id_to_tag_id_pair.contains(capture_id)) {
            return m_capture_id_to_tag_id_pair.at(capture_id);
        }
        return std::nullopt;
    }

    /**
     * @param tag_id ID associated with a tag.
     * @return The final register ID tracking the value of the tag ID during DFA simulation on
     * success.
     * @return std::nullopt if no tag is associated with the given tag ID.
     */
    [[nodiscard]] auto get_reg_id_from_tag_id(tag_id_t const tag_id) const
            -> std::optional<reg_id_t> {
        auto const& tag_id_to_final_reg_id{m_dfa->get_tag_id_to_final_reg_id()};
        if (tag_id_to_final_reg_id.contains(tag_id)) {
            return tag_id_to_final_reg_id.at(tag_id);
        }
        return std::nullopt;
    }

    /**
     * Retrieves the register IDs for the start and end tags associated with a given capture ID.
     * @param capture_id The ID of the capture to search for.
     * @return A pair of register IDs corresponding to the start and end tags of the capture.
     * @return std::nullopt if no such capture is found.
     */
    [[nodiscard]] auto get_reg_ids_from_capture_id(capture_id_t const capture_id) const
            -> std::optional<std::pair<reg_id_t, reg_id_t>> {
        auto const optional_tag_id_pair{get_tag_id_pair_from_capture_id(capture_id)};
        if (false == optional_tag_id_pair.has_value()) {
            return std::nullopt;
        }
        auto const [start_tag_id, end_tag_id]{optional_tag_id_pair.value()};

        auto const optional_start_reg_id{get_reg_id_from_tag_id(start_tag_id)};
        if (false == optional_start_reg_id.has_value()) {
            return std::nullopt;
        }

        auto const optional_end_reg_id{get_reg_id_from_tag_id(end_tag_id)};
        if (false == optional_end_reg_id.has_value()) {
            return std::nullopt;
        }

        return std::make_pair(optional_start_reg_id.value(), optional_end_reg_id.value());
    }

    std::unordered_map<std::string, rule_id_t> m_symbol_id;
    std::unordered_map<rule_id_t, std::string> m_id_symbol;

private:
    /**
     * @return The nexer character from the input buffer.
     */
    [[nodiscard]] auto get_next_character() -> unsigned char;

    uint32_t m_match_pos{0};
    uint32_t m_start_pos{0};
    uint32_t m_match_line{0};
    uint32_t m_last_match_pos{0};
    uint32_t m_last_match_line{0};
    bool m_match{false};
    std::vector<uint32_t> const* m_type_ids{nullptr};
    std::set<uint32_t> m_type_ids_set;
    std::array<bool, cSizeOfByte> m_is_delimiter{false};
    std::array<bool, cSizeOfByte> m_is_first_char_of_a_variable{false};
    std::vector<LexicalRule<TypedNfaState>> m_rules;
    uint32_t m_line{0};
    bool m_has_delimiters{false};
    std::unique_ptr<finite_automata::Dfa<TypedDfaState, TypedNfaState>> m_dfa;
    std::optional<uint32_t> m_optional_first_delimiter_pos{std::nullopt};
    bool m_asked_for_more_data{false};
    TypedDfaState const* m_prev_state{nullptr};
    std::unordered_map<rule_id_t, std::vector<capture_id_t>> m_rule_id_to_capture_ids;
    std::unordered_map<capture_id_t, std::pair<tag_id_t, tag_id_t>> m_capture_id_to_tag_id_pair;
};

namespace lexers {
using ByteLexer = Lexer<finite_automata::ByteNfaState, finite_automata::ByteDfaState>;
using Utf8Lexer = Lexer<finite_automata::Utf8NfaState, finite_automata::Utf8DfaState>;
}  // namespace lexers
}  // namespace log_surgeon

#include "Lexer.tpp"

#endif  // LOG_SURGEON_LEXER_HPP
