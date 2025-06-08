#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP

#include <algorithm>
#include <array>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <ranges>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Capture.hpp>
#include <log_surgeon/finite_automata/TagOperation.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

#include <fmt/core.h>
#include <fmt/ranges.h>
#include <fmt/xchar.h>
#include <gsl/pointers>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
class Nfa;

// TODO: rename `RegexAST` to `RegexASTNode`
/**
 * Base class for a Regex AST node.
 * Unique capture pointers are used to differentiate each capture group node. Every node will
 * maintain two sets of captures:
 * 1. `m_subtree_positive_captures`: the set of captures matched by all capture groups within the
 *    subtree rooted at this node.
 * 2. `m_negative_captures`: the set of captures that are guaranteed to be unmatched when traversing
 *    this node, as the alternative path contains these captures.
 *
 * ASTs built using this class are assumed to be constructed in a bottom-up manner, where all
 * descendant nodes are created first.
 *
 * @tparam TypedNfaState Whether this AST is used for byte lexing or UTF-8 lexing.
 */
template <typename TypedNfaState>
class RegexAST {
public:
    RegexAST() = default;

    virtual ~RegexAST() = default;

    /**
     * Used for cloning a unique_pointer of base type RegexAST
     * @return RegexAST*
     */
    [[nodiscard]] virtual auto clone() const -> gsl::owner<RegexAST*> = 0;

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule
     * @param is_possible_input
     */
    virtual auto
    set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const -> void
            = 0;

    /**
     * transform '.' from any-character into any non-delimiter in a lexer rule
     * @param delimiters
     */
    virtual auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void = 0;

    /**
     * Adds the necessary `Nfa::states` to the passed-in NFA to handle the current node, before
     * transitioning to the accepting `end_state`.
     *
     * This method also tracks whether the current node is inside a repetition (e.g., `*`, `+`,
     * `{m,n}`). If the node is within a repetition context, it will alter its behavior accordingly
     * (e.g., changing tag operations).
     *
     * @param nfa The NFA to which the states will be added.
     * @param end_state The state that represents the accepting state for the node.
     * @param descendent_of_repetition  A flag indicating whether the current node is a descendent
     * of a repetition.
     */
    virtual auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void
            = 0;

    /**
     * Serializes the AST with this node as the root.
     * @return A string representing the serialized AST.
     */
    [[nodiscard]] virtual auto serialize() const -> std::u32string = 0;

    [[nodiscard]] auto get_subtree_positive_captures() const -> std::vector<Capture const*> const& {
        return m_subtree_positive_captures;
    }

    auto set_subtree_positive_captures(std::vector<Capture const*> subtree_positive_captures)
            -> void {
        m_subtree_positive_captures = std::move(subtree_positive_captures);
    }

    auto add_subtree_positive_captures(std::vector<Capture const*> const& subtree_positive_captures)
            -> void {
        m_subtree_positive_captures.insert(
                m_subtree_positive_captures.end(),
                subtree_positive_captures.cbegin(),
                subtree_positive_captures.cend()
        );
    }

    auto set_negative_captures(std::vector<Capture const*> negative_captures) -> void {
        m_negative_captures = std::move(negative_captures);
    }

    /**
     * Handles the addition of an intermediate state with a negative transition if needed, while
     * taking into account whether the current node is inside a repetition context.
     *
     * This method constructs the necessary transitions, including negative captures, and alters its
     * behavior depending on whether the node is a descendent of a repetition (e.g., `*`, `+`,
     * `{m,n}`), adjusting transitions accordingly.
     *
     * @param nfa The NFA to which the states will be added.
     * @param end_state The state representing the accepting end state for the node.
     * @param descendent_of_repetition  A flag indicating if the current node is inside a repetition
     * context.
     */
    auto add_to_nfa_with_negative_captures(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool const descendent_of_repetition
    ) const -> void {
        // Handle negative captures as:
        // root --(regex)--> state_with_spontaneous_transition --(negate tags)--> end_state
        if (false == m_negative_captures.empty()) {
            auto* state_with_spontaneous_transition{nfa->new_state_from_negative_captures(
                    m_negative_captures,
                    end_state,
                    descendent_of_repetition
            )};
            add_to_nfa(nfa, state_with_spontaneous_transition, descendent_of_repetition);
        } else {
            add_to_nfa(nfa, end_state, descendent_of_repetition);
        }
    }

protected:
    RegexAST(RegexAST const& rhs) = default;
    auto operator=(RegexAST const& rhs) -> RegexAST& = default;
    RegexAST(RegexAST&& rhs) noexcept = delete;
    auto operator=(RegexAST&& rhs) noexcept -> RegexAST& = delete;

    [[nodiscard]] auto serialize_negative_captures() const -> std::u32string {
        if (m_negative_captures.empty()) {
            return U"";
        }

        auto const transformed_negative_captures{
                m_negative_captures | std::ranges::views::transform([](Capture const* capture) {
                    return fmt::format("<~{}>", capture->get_name());
                })
        };
        auto const negative_captures_string{
                fmt::format("{}", fmt::join(transformed_negative_captures, ""))
        };

        return fmt::format(
                U"{}",
                std::u32string(negative_captures_string.begin(), negative_captures_string.end())
        );
    }

private:
    std::vector<Capture const*> m_subtree_positive_captures;
    std::vector<Capture const*> m_negative_captures;
};

/**
 * Class for an empty AST node. This is used to simplify tagged-NFA creation when using regex
 * repetition with a minimum repetition of 0. Namely, we treat `R{0,N}` as `R{1,N} | ∅`. Then, the
 * NFA handles the 0 repetition case using the logic in `RegexASTOR` (i.e., adding a negative
 * transition for every capture group matched in `R{1,N}`).
 * @tparam TypedNfaState Whether this AST is used for byte lexing or UTF-8 lexing.
 */
template <typename TypedNfaState>
class RegexASTEmpty : public RegexAST<TypedNfaState> {
public:
    RegexASTEmpty() = default;

    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTEmpty*> override {
        return new RegexASTEmpty(*this);
    }

    auto set_possible_inputs_to_true(
            [[maybe_unused]] std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
        // Do nothing as an empty node contains no utf8 characters.
    }

    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters)
            -> void override {
        // Do nothing as an empty node contains no delimiters.
    }

    auto add_to_nfa(
            [[maybe_unused]] Nfa<TypedNfaState>* nfa,
            [[maybe_unused]] TypedNfaState* end_state,
            [[maybe_unused]] bool const descendent_of_repetition
    ) const -> void override {
        nfa->get_root()->add_spontaneous_transition(end_state);
    }

    [[nodiscard]] auto serialize() const -> std::u32string override;
};

template <typename TypedNfaState>
class RegexASTLiteral : public RegexAST<TypedNfaState> {
public:
    explicit RegexASTLiteral(uint32_t character);

    /**
     * Used for cloning a unique_pointer of type RegexASTLiteral
     * @return RegexASTLiteral*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTLiteral*> override {
        return new RegexASTLiteral(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTLiteral at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        is_possible_input[m_character] = true;
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTLiteral is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters)
            -> void override {
        // Do nothing
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_character() const -> uint32_t const& { return m_character; }

private:
    uint32_t m_character;
};

template <typename TypedNfaState>
class RegexASTInteger : public RegexAST<TypedNfaState> {
public:
    explicit RegexASTInteger(uint32_t digit);

    RegexASTInteger(RegexASTInteger* left, uint32_t digit);

    /**
     * Used for cloning a unique_pointer of type RegexASTInteger
     * @return RegexASTInteger*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTInteger*> override {
        return new RegexASTInteger(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTInteger at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        for (uint32_t const i : m_digits) {
            is_possible_input.at('0' + i) = true;
        }
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTInteger is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters)
            -> void override {
        // Do nothing
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_digits() const -> std::vector<uint32_t> const& { return m_digits; }

    [[nodiscard]] auto get_digit(uint32_t i) const -> uint32_t const& { return m_digits[i]; }

private:
    std::vector<uint32_t> m_digits;
};

template <typename TypedNfaState>
class RegexASTGroup : public RegexAST<TypedNfaState> {
public:
    using Range = std::pair<uint32_t, uint32_t>;

    RegexASTGroup() = default;

    explicit RegexASTGroup(RegexASTLiteral<TypedNfaState> const* right);

    explicit RegexASTGroup(RegexASTGroup const* right);

    RegexASTGroup(RegexASTGroup const* left, RegexASTLiteral<TypedNfaState> const* right);

    RegexASTGroup(RegexASTGroup const* left, RegexASTGroup const* right);

    RegexASTGroup(
            RegexASTLiteral<TypedNfaState> const* left,
            RegexASTLiteral<TypedNfaState> const* right
    );

    RegexASTGroup(uint32_t min, uint32_t max);

    explicit RegexASTGroup(std::vector<uint32_t> const& literals);

    /**
     * Used for cloning a unique_pointer of type RegexASTGroup
     * @return RegexASTGroup*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTGroup*> override {
        return new RegexASTGroup(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTGroup at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        if (!m_negate) {
            for (auto const& [begin, end] : m_ranges) {
                for (uint32_t i = begin; i <= end; i++) {
                    is_possible_input.at(i) = true;
                }
            }
        } else {
            std::vector<char> inputs(cSizeOfUnicode, 1);
            for (auto const& [begin, end] : m_ranges) {
                for (uint32_t i = begin; i <= end; i++) {
                    inputs[i] = 0;
                }
            }
            for (uint32_t i = 0; i < inputs.size(); i++) {
                if (inputs[i] != 0) {
                    is_possible_input.at(i) = true;
                }
            }
        }
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if this
     * RegexASTGroup node contains `.` (is a wildcard group)
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        if (!m_is_wildcard) {
            return;
        }
        if (delimiters.empty()) {
            return;
        }
        m_ranges.clear();
        std::ranges::sort(delimiters);
        if (delimiters[0] != 0) {
            Range const range(0, delimiters[0] - 1);
            m_ranges.push_back(range);
        }
        for (uint32_t i = 1; i < delimiters.size(); i++) {
            if (delimiters[i] - delimiters[i - 1] > 1) {
                Range const range(delimiters[i - 1] + 1, delimiters[i] - 1);
                m_ranges.push_back(range);
            }
        }
        if (delimiters.back() != cUnicodeMax) {
            Range const range(delimiters.back() + 1, cUnicodeMax);
            m_ranges.push_back(range);
        }
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    auto add_range(uint32_t min, uint32_t max) -> void { m_ranges.emplace_back(min, max); }

    auto add_literal(uint32_t literal) -> void { m_ranges.emplace_back(literal, literal); }

    auto set_is_wildcard_true() -> void { m_is_wildcard = true; }

    [[nodiscard]] auto is_wildcard() const -> bool { return m_is_wildcard; }

    [[nodiscard]] auto is_negated() const -> bool { return m_negate; }

    [[nodiscard]] auto get_ranges() const -> std::vector<Range> { return m_ranges; }

private:
    /**
     * Merges multiple ranges such that the resulting m_ranges is sorted and
     * non-overlapping @param ranges
     * @return std::vector<Range>
     */
    static auto merge(std::vector<Range> const& ranges) -> std::vector<Range>;

    /**
     * Takes the compliment (in the cast of regex `^` at the start of a group)
     * of multiple ranges such that m_ranges is sorted and non-overlapping
     * @param ranges
     * @return std::vector<Range>
     */
    static auto complement(std::vector<Range> const& ranges) -> std::vector<Range>;

    bool m_is_wildcard{false};
    bool m_negate{true};
    std::vector<Range> m_ranges;
};

template <typename TypedNfaState>
class RegexASTOr : public RegexAST<TypedNfaState> {
public:
    ~RegexASTOr() override = default;

    RegexASTOr(
            std::unique_ptr<RegexAST<TypedNfaState>> left,
            std::unique_ptr<RegexAST<TypedNfaState>> right
    );

    RegexASTOr(RegexASTOr const& rhs)
            : RegexAST<TypedNfaState>(rhs),
              m_left(std::unique_ptr<RegexAST<TypedNfaState>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<TypedNfaState>>(rhs.m_right->clone())) {}

    /**
     * Used for cloning a unique_pointer of type RegexASTOr
     * @return RegexASTOr*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTOr*> override {
        return new RegexASTOr(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTOr at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        m_left->set_possible_inputs_to_true(is_possible_input);
        m_right->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * RegexASTGroup with `.` is a descendant of this RegexASTOr node
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_left->remove_delimiters_from_wildcard(delimiters);
        m_right->remove_delimiters_from_wildcard(delimiters);
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_left() const -> RegexAST<TypedNfaState> const* { return m_left.get(); }

    [[nodiscard]] auto get_right() const -> RegexAST<TypedNfaState> const* { return m_right.get(); }

private:
    std::unique_ptr<RegexAST<TypedNfaState>> m_left;
    std::unique_ptr<RegexAST<TypedNfaState>> m_right;
};

template <typename TypedNfaState>
class RegexASTCat : public RegexAST<TypedNfaState> {
public:
    ~RegexASTCat() override = default;

    RegexASTCat(
            std::unique_ptr<RegexAST<TypedNfaState>> left,
            std::unique_ptr<RegexAST<TypedNfaState>> right
    );

    RegexASTCat(RegexASTCat const& rhs)
            : RegexAST<TypedNfaState>(rhs),
              m_left(std::unique_ptr<RegexAST<TypedNfaState>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<TypedNfaState>>(rhs.m_right->clone())) {}

    /**
     * Used for cloning a unique_pointer of type RegexASTCat
     * @return RegexASTCat*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTCat*> override {
        return new RegexASTCat(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTCat at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        m_left->set_possible_inputs_to_true(is_possible_input);
        m_right->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * RegexASTGroup with `.` is a descendant of this RegexASTCat node
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_left->remove_delimiters_from_wildcard(delimiters);
        m_right->remove_delimiters_from_wildcard(delimiters);
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_left() const -> RegexAST<TypedNfaState> const* { return m_left.get(); }

    [[nodiscard]] auto get_right() const -> RegexAST<TypedNfaState> const* { return m_right.get(); }

private:
    std::unique_ptr<RegexAST<TypedNfaState>> m_left;
    std::unique_ptr<RegexAST<TypedNfaState>> m_right;
};

template <typename TypedNfaState>
class RegexASTMultiplication : public RegexAST<TypedNfaState> {
public:
    ~RegexASTMultiplication() override = default;

    RegexASTMultiplication(
            std::unique_ptr<RegexAST<TypedNfaState>> operand,
            uint32_t min,
            uint32_t max
    );

    RegexASTMultiplication(RegexASTMultiplication const& rhs)
            : RegexAST<TypedNfaState>(rhs),
              m_operand(std::unique_ptr<RegexAST<TypedNfaState>>(rhs.m_operand->clone())),
              m_min(rhs.m_min),
              m_max(rhs.m_max) {}

    /**
     * Used for cloning a unique_pointer of type RegexASTMultiplication
     * @return RegexASTMultiplication*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTMultiplication*> override {
        return new RegexASTMultiplication(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTMultiplication at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        m_operand->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * RegexASTGroup with `.` is a descendant of this RegexASTMultiplication
     * node
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_operand->remove_delimiters_from_wildcard(delimiters);
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* end_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto is_infinite() const -> bool { return m_max == 0; }

    [[nodiscard]] auto get_operand() const -> std::unique_ptr<RegexAST<TypedNfaState>> const& {
        return m_operand;
    }

    [[nodiscard]] auto get_min() const -> uint32_t { return m_min; }

    [[nodiscard]] auto get_max() const -> uint32_t { return m_max; }

private:
    std::unique_ptr<RegexAST<TypedNfaState>> m_operand;
    uint32_t m_min;
    uint32_t m_max;
};

/**
 * Represents a capture group AST node.
 * NOTE:
 * - `m_capture` must be non-null as it represents the capture group being matched.
 * - `m_capture_regex_ast` must be non-null as it contains the regex pattern for the capture group.
 * @tparam TypedNfaState Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename TypedNfaState>
class RegexASTCapture : public RegexAST<TypedNfaState> {
public:
    ~RegexASTCapture() override = default;

    /**
     * @param capture_regex_ast
     * @param capture
     * @throw std::invalid_argument if `capture_regex_ast` or `capture` are `nullptr`.
     */
    RegexASTCapture(
            std::unique_ptr<RegexAST<TypedNfaState>> capture_regex_ast,
            std::unique_ptr<Capture> capture
    )
            : m_capture_regex_ast{std::move(capture_regex_ast)},
              m_capture{std::move(capture)} {
        if (nullptr == m_capture_regex_ast) {
            throw std::invalid_argument("Group regex AST cannot be null");
        }
        if (nullptr == m_capture) {
            throw std::invalid_argument("Capture cannot be null");
        }

        RegexAST<TypedNfaState>::set_subtree_positive_captures(
                m_capture_regex_ast->get_subtree_positive_captures()
        );
        RegexAST<TypedNfaState>::add_subtree_positive_captures({m_capture.get()});
    }

    RegexASTCapture(RegexASTCapture const& rhs)
            : RegexAST<TypedNfaState>{rhs},
              m_capture_regex_ast{
                      std::unique_ptr<RegexAST<TypedNfaState>>(rhs.m_capture_regex_ast->clone())
              },
              m_capture{std::make_unique<Capture>(*rhs.m_capture)} {
        RegexAST<TypedNfaState>::set_subtree_positive_captures(rhs.get_subtree_positive_captures());
    }

    /**
     * Used for cloning a `unique_pointer` of type `RegexASTCapture`.
     * @return RegexASTCapture*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTCapture*> override {
        return new RegexASTCapture(*this);
    }

    /**
     * Sets `is_possible_input` to specify which utf8 characters are allowed in a
     * lexer rule containing `RegexASTCapture` at a leaf node in its AST.
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input) const
            -> void override {
        m_capture_regex_ast->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * `RegexASTGroup` with `.` is a descendant of this `RegexASTCapture` node.
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_capture_regex_ast->remove_delimiters_from_wildcard(delimiters);
    }

    auto add_to_nfa(
            Nfa<TypedNfaState>* nfa,
            TypedNfaState* dest_state,
            bool descendent_of_repetition
    ) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_capture_name() const -> std::string_view {
        return m_capture->get_name();
    }

    [[nodiscard]] auto get_capture_regex_ast() const
            -> std::unique_ptr<RegexAST<TypedNfaState>> const& {
        return m_capture_regex_ast;
    }

private:
    std::unique_ptr<RegexAST<TypedNfaState>> m_capture_regex_ast;
    std::unique_ptr<Capture> m_capture;
};

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTEmpty<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(U"{}", RegexAST<TypedNfaState>::serialize_negative_captures());
}

template <typename TypedNfaState>
RegexASTLiteral<TypedNfaState>::RegexASTLiteral(uint32_t character) : m_character(character) {}

template <typename TypedNfaState>
void RegexASTLiteral<TypedNfaState>::add_to_nfa(
        Nfa<TypedNfaState>* nfa,
        TypedNfaState* end_state,
        [[maybe_unused]] bool const descendent_of_repetition
) const {
    nfa->add_root_interval(Interval(m_character, m_character), end_state);
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTLiteral<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}",
            static_cast<char32_t>(m_character),
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}

template <typename TypedNfaState>
RegexASTInteger<TypedNfaState>::RegexASTInteger(uint32_t digit) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename TypedNfaState>
RegexASTInteger<TypedNfaState>::RegexASTInteger(RegexASTInteger* left, uint32_t digit)
        : m_digits(std::move(left->m_digits)) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename TypedNfaState>
void RegexASTInteger<TypedNfaState>::add_to_nfa(
        [[maybe_unused]] Nfa<TypedNfaState>* nfa,
        [[maybe_unused]] TypedNfaState* end_state,
        [[maybe_unused]] bool const descendent_of_repetition
) const {
    throw std::runtime_error("Unsupported");
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTInteger<TypedNfaState>::serialize() const -> std::u32string {
    auto const digits_string = fmt::format("{}", fmt::join(m_digits, ""));
    return fmt::format(
            U"{}{}",
            std::u32string(digits_string.begin(), digits_string.end()),
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}

template <typename TypedNfaState>
RegexASTOr<TypedNfaState>::RegexASTOr(
        std::unique_ptr<RegexAST<TypedNfaState>> left,
        std::unique_ptr<RegexAST<TypedNfaState>> right
)
        : m_left(std::move(left)),
          m_right(std::move(right)) {
    m_left->set_negative_captures(m_right->get_subtree_positive_captures());
    m_right->set_negative_captures(m_left->get_subtree_positive_captures());
    RegexAST<TypedNfaState>::set_subtree_positive_captures(m_left->get_subtree_positive_captures());
    RegexAST<TypedNfaState>::add_subtree_positive_captures(
            m_right->get_subtree_positive_captures()
    );
}

template <typename TypedNfaState>
void RegexASTOr<TypedNfaState>::add_to_nfa(
        Nfa<TypedNfaState>* nfa,
        TypedNfaState* end_state,
        bool const descendent_of_repetition
) const {
    m_left->add_to_nfa_with_negative_captures(nfa, end_state, descendent_of_repetition);
    m_right->add_to_nfa_with_negative_captures(nfa, end_state, descendent_of_repetition);
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTOr<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"({})|({}){}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}

template <typename TypedNfaState>
RegexASTCat<TypedNfaState>::RegexASTCat(
        std::unique_ptr<RegexAST<TypedNfaState>> left,
        std::unique_ptr<RegexAST<TypedNfaState>> right
)
        : m_left(std::move(left)),
          m_right(std::move(right)) {
    RegexAST<TypedNfaState>::set_subtree_positive_captures(m_left->get_subtree_positive_captures());
    RegexAST<TypedNfaState>::add_subtree_positive_captures(
            m_right->get_subtree_positive_captures()
    );
}

template <typename TypedNfaState>
void RegexASTCat<TypedNfaState>::add_to_nfa(
        Nfa<TypedNfaState>* nfa,
        TypedNfaState* end_state,
        bool const descendent_of_repetition
) const {
    TypedNfaState* saved_root = nfa->get_root();
    TypedNfaState* intermediate_state = nfa->new_state();
    m_left->add_to_nfa_with_negative_captures(nfa, intermediate_state, descendent_of_repetition);
    nfa->set_root(intermediate_state);
    m_right->add_to_nfa_with_negative_captures(nfa, end_state, descendent_of_repetition);
    nfa->set_root(saved_root);
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTCat<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}{}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}

template <typename TypedNfaState>
RegexASTMultiplication<TypedNfaState>::RegexASTMultiplication(
        std::unique_ptr<RegexAST<TypedNfaState>> operand,
        uint32_t const min,
        uint32_t const max
)
        : m_operand(std::move(operand)),
          m_min(min),
          m_max(max) {
    RegexAST<TypedNfaState>::set_subtree_positive_captures(
            m_operand->get_subtree_positive_captures()
    );
}

template <typename TypedNfaState>
void RegexASTMultiplication<TypedNfaState>::add_to_nfa(
        Nfa<TypedNfaState>* nfa,
        TypedNfaState* end_state,
        [[maybe_unused]] bool const descendent_of_repetition
) const {
    TypedNfaState* saved_root = nfa->get_root();
    if (m_min == 0) {
        nfa->get_root()->add_spontaneous_transition(end_state);
    } else {
        for (uint32_t i{1}; i < m_min; ++i) {
            TypedNfaState* intermediate_state = nfa->new_state();
            m_operand->add_to_nfa_with_negative_captures(nfa, intermediate_state, true);
            nfa->set_root(intermediate_state);
        }
        m_operand->add_to_nfa_with_negative_captures(nfa, end_state, true);
    }
    if (is_infinite()) {
        nfa->set_root(end_state);
        m_operand->add_to_nfa_with_negative_captures(nfa, end_state, true);
    } else if (m_max > m_min) {
        if (m_min != 0) {
            TypedNfaState* intermediate_state = nfa->new_state();
            m_operand->add_to_nfa_with_negative_captures(nfa, intermediate_state, true);
            nfa->set_root(intermediate_state);
        }
        for (uint32_t i = m_min + 1; i < m_max; ++i) {
            m_operand->add_to_nfa_with_negative_captures(nfa, end_state, true);
            TypedNfaState* intermediate_state = nfa->new_state();
            m_operand->add_to_nfa_with_negative_captures(nfa, intermediate_state, true);
            nfa->set_root(intermediate_state);
        }
        m_operand->add_to_nfa_with_negative_captures(nfa, end_state, true);
    }
    nfa->set_root(saved_root);
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTMultiplication<TypedNfaState>::serialize() const -> std::u32string {
    auto const min_string = std::to_string(m_min);
    auto const max_string = std::to_string(m_max);

    return fmt::format(
            U"({}){{{},{}}}{}",
            nullptr != m_operand ? m_operand->serialize() : U"null",
            std::u32string(min_string.begin(), min_string.end()),
            is_infinite() ? U"inf" : std::u32string(max_string.begin(), max_string.end()),
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}

template <typename TypedNfaState>
auto RegexASTCapture<TypedNfaState>::add_to_nfa(
        Nfa<TypedNfaState>* nfa,
        TypedNfaState* dest_state,
        bool const descendent_of_repetition
) const -> void {
    // TODO: move this into a documentation file in the future, and reference it here.
    // The NFA constructed for a capture group follows the structure below, with spontaneous
    // transitions explicitly labeled for clarity:
    //         +---------------------+
    //         |       `m_root`      |
    //         +---------------------+
    //                    |
    //                    | (set start tag position for `m_capture`)
    //                    v
    //         +---------------------+
    //         |`capture_start_state`|
    //         +---------------------+
    //                    |
    //                    | (no tag operation)
    //                    v
    //         +---------------------+
    //         |`m_capture_regex_ast`|
    //         |    (nested NFA)     |
    //         +---------------------+
    //                    | `m_negative_captures`
    //                    | (negate alternate tags)
    //                    v
    //         +---------------------+
    //         | `capture_end_state` |
    //         +---------------------+
    //                    |
    //                    | (set end tag position for `m_capture`)
    //                    v
    //         +---------------------+
    //         |     `dest_state`    |
    //         +---------------------+
    auto [capture_start_state,
          capture_end_state]{nfa->new_start_and_end_states_from_positive_capture(
            m_capture.get(),
            dest_state,
            descendent_of_repetition
    )};

    auto* initial_root = nfa->get_root();
    nfa->set_root(capture_start_state);
    m_capture_regex_ast
            ->add_to_nfa_with_negative_captures(nfa, capture_end_state, descendent_of_repetition);
    nfa->set_root(initial_root);
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTCapture<TypedNfaState>::serialize() const -> std::u32string {
    auto const capture_name_u32{
            std::u32string(m_capture->get_name().cbegin(), m_capture->get_name().cend())
    };
    return fmt::format(
            U"({})<{}>{}",
            m_capture_regex_ast->serialize(),
            capture_name_u32,
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(
        RegexASTGroup const* left,
        RegexASTLiteral<TypedNfaState> const* right
) {
    if (right == nullptr) {
        throw std::runtime_error(
                "RegexASTGroup1: right == nullptr: A bracket expression in the schema contains"
                " illegal characters, remember to escape special characters. Refer to"
                " README-Schema.md for more details."
        );
    }
    m_negate = left->m_negate;
    m_ranges = left->m_ranges;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(RegexASTGroup const* left, RegexASTGroup const* right)
        : m_negate(left->m_negate),
          m_ranges(left->m_ranges) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(RegexASTLiteral<TypedNfaState> const* right) {
    if (right == nullptr) {
        throw std::runtime_error(
                "RegexASTGroup2: right == nullptr: A bracket expression in the schema contains"
                " illegal characters, remember to escape special characters. Refer to"
                " README-Schema.md for more details."
        );
    }
    m_negate = false;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(RegexASTGroup const* right) : m_negate(false) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(
        RegexASTLiteral<TypedNfaState> const* left,
        RegexASTLiteral<TypedNfaState> const* right
) {
    if (left == nullptr || right == nullptr) {
        throw std::runtime_error(
                "RegexASTGroup3: left == nullptr || right == nullptr: A bracket expression in the "
                "schema contains illegal characters, remember to escape special characters. Refer "
                "to README-Schema.md for more details."
        );
    }
    m_negate = false;
    assert(right->get_character() > left->get_character());
    m_ranges.emplace_back(left->get_character(), right->get_character());
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(std::vector<uint32_t> const& literals)
        : m_negate(false) {
    for (uint32_t literal : literals) {
        m_ranges.emplace_back(literal, literal);
    }
}

template <typename TypedNfaState>
RegexASTGroup<TypedNfaState>::RegexASTGroup(uint32_t min, uint32_t max) : m_negate(false) {
    m_ranges.emplace_back(min, max);
}

// ranges must be sorted
template <typename TypedNfaState>
auto RegexASTGroup<TypedNfaState>::merge(std::vector<Range> const& ranges) -> std::vector<Range> {
    std::vector<Range> merged_ranges;
    if (ranges.empty()) {
        return merged_ranges;
    }
    Range cur = ranges[0];
    for (size_t i = 1; i < ranges.size(); i++) {
        auto const& range = ranges[i];
        if (range.first <= cur.second + 1) {
            cur.second = std::max(range.second, cur.second);
        } else {
            merged_ranges.push_back(cur);
            cur = range;
        }
    }
    merged_ranges.push_back(cur);
    return merged_ranges;
}

// ranges must be sorted and non-overlapping
template <typename TypedNfaState>
auto RegexASTGroup<TypedNfaState>::complement(std::vector<Range> const& ranges)
        -> std::vector<Range> {
    std::vector<Range> complemented;
    uint32_t low = 0;
    for (auto const& [begin, end] : ranges) {
        if (begin > 0) {
            complemented.emplace_back(low, begin - 1);
        }
        low = end + 1;
    }
    if (low > 0) {
        complemented.emplace_back(low, cUnicodeMax);
    }
    return complemented;
}

template <typename TypedNfaState>
void RegexASTGroup<TypedNfaState>::add_to_nfa(
        Nfa<TypedNfaState>* nfa,
        TypedNfaState* end_state,
        [[maybe_unused]] bool const descendent_of_repetition
) const {
    // TODO: there should be a better way to do this with a set and keep m_ranges sorted, but we
    // have to consider removing overlap + taking the compliment.
    auto merged_ranges = m_ranges;
    std::sort(merged_ranges.begin(), merged_ranges.end());
    merged_ranges = merge(merged_ranges);
    if (m_negate) {
        merged_ranges = complement(merged_ranges);
    }
    for (auto const& [begin, end] : merged_ranges) {
        nfa->get_root()->add_interval(Interval(begin, end), end_state);
    }
}

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTGroup<TypedNfaState>::serialize() const -> std::u32string {
    std::u32string ranges_serialized;
    if (m_is_wildcard) {
        ranges_serialized += U"*";
    } else {
        auto const transformed_ranges
                = m_ranges
                  | std::ranges::views::transform([](std::pair<uint32_t, uint32_t> const& range) {
                        auto const [begin, end] = range;
                        return fmt::format(
                                U"{}-{}",
                                static_cast<char32_t>(begin),
                                static_cast<char32_t>(end)
                        );
                    });
        for (auto const& range_u32string : transformed_ranges) {
            if (false == ranges_serialized.empty()) {
                ranges_serialized += U", ";  // Add separator
            }
            ranges_serialized += range_u32string;
        }
    }
    return fmt::format(
            U"[{}{}]{}",
            m_negate ? U"^" : U"",
            ranges_serialized,
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
