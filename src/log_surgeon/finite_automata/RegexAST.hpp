#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP

#include <algorithm>
#include <array>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <gsl/pointers>
#include <memory>
#include <ranges>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <fmt/core.h>
#include <fmt/ranges.h>
#include <fmt/xchar.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Tag.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
template <typename NfaStateType>
class Nfa;

// TODO: rename `RegexAST` to `RegexASTNode`
/**
 * Base class for a Regex AST node.
 * Unique integer tags are used to differentiate each capture group node. Every node will maintain
 * two sets of tags:
 * 1. `m_subtree_positive_tags`: the set of tags matched by all capture groups within the subtree
 *    rooted at this node.
 * 2. `m_negative_tags`: the set of tags that are guaranteed to be unmatched when traversing this
 *    node, as the alternative path contains these tags.
 *
 * ASTs built using this class are assumed to be constructed in a bottom-up manner, where all
 * descendant nodes are created first.
 *
 * @tparam NfaStateType Whether this AST is used for byte lexing or UTF-8 lexing.
 */
template <typename NfaStateType>
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
    virtual auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void = 0;

    /**
     * transform '.' from any-character into any non-delimiter in a lexer rule
     * @param delimiters
     */
    virtual auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void = 0;

    /**
     * Add the needed Nfa::states to the passed in nfa to handle the
     * current node before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    virtual auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void = 0;

    /**
     * Serializes the AST with this node as the root.
     * @return A string representing the serialized AST.
     */
    [[nodiscard]] virtual auto serialize() const -> std::u32string = 0;

    [[nodiscard]] auto get_subtree_positive_tags() const -> std::vector<Tag const*> const& {
        return m_subtree_positive_tags;
    }

    auto set_subtree_positive_tags(std::vector<Tag const*> subtree_positive_tags) -> void {
        m_subtree_positive_tags = std::move(subtree_positive_tags);
    }

    auto add_subtree_positive_tags(std::vector<Tag const*> const& subtree_positive_tags) -> void {
        m_subtree_positive_tags.insert(
                m_subtree_positive_tags.end(),
                subtree_positive_tags.cbegin(),
                subtree_positive_tags.cend()
        );
    }

    auto set_negative_tags(std::vector<Tag const*> negative_tags) -> void {
        m_negative_tags = std::move(negative_tags);
    }

    /**
     * Handles the addition of an intermediate state with a negative transition if needed.
     * @param nfa
     * @param end_state
     */
    auto
    add_to_nfa_with_negative_tags(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void {
        // Handle negative tags as:
        // root --(regex)--> state_with_negative_tagged_transition --(negative tags)--> end_state
        if (false == m_negative_tags.empty()) {
            auto* state_with_negative_tagged_transition
                    = nfa->new_state_with_negative_tagged_transition(m_negative_tags, end_state);
            add_to_nfa(nfa, state_with_negative_tagged_transition);
        } else {
            add_to_nfa(nfa, end_state);
        }
    }

protected:
    RegexAST(RegexAST const& rhs) = default;
    auto operator=(RegexAST const& rhs) -> RegexAST& = default;
    RegexAST(RegexAST&& rhs) noexcept = delete;
    auto operator=(RegexAST&& rhs) noexcept -> RegexAST& = delete;

    [[nodiscard]] auto serialize_negative_tags() const -> std::u32string {
        if (m_negative_tags.empty()) {
            return U"";
        }

        auto const transformed_negative_tags
                = m_negative_tags | std::ranges::views::transform([](Tag const* tag) {
                      return fmt::format("<~{}>", tag->get_name());
                  });
        auto const negative_tags_string
                = fmt::format("{}", fmt::join(transformed_negative_tags, ""));

        return fmt::format(
                U"{}",
                std::u32string(negative_tags_string.begin(), negative_tags_string.end())
        );
    }

private:
    std::vector<Tag const*> m_subtree_positive_tags;
    std::vector<Tag const*> m_negative_tags;
};

/**
 * Class for an empty AST node. This is used to simplify tagged-NFA creation when using regex
 * repetition with a minimum repetition of 0. Namely, we treat `R{0,N}` as `R{1,N} | ∅`. Then, the
 * NFA handles the 0 repetition case using the logic in `RegexASTOR` (i.e., adding a negative
 * transition for every capture group matched in `R{1,N}`).
 * @tparam NfaStateType Whether this AST is used for byte lexing or UTF-8 lexing.
 */
template <typename NfaStateType>
class RegexASTEmpty : public RegexAST<NfaStateType> {
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

    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters
    ) -> void override {
        // Do nothing as an empty node contains no delimiters.
    }

    auto add_to_nfa(
            [[maybe_unused]] Nfa<NfaStateType>* nfa,
            [[maybe_unused]] NfaStateType* end_state
    ) const -> void override {
        // Do nothing as adding an empty node to the NFA is a null operation.
    }

    [[nodiscard]] auto serialize() const -> std::u32string override;
};

template <typename NfaStateType>
class RegexASTLiteral : public RegexAST<NfaStateType> {
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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
        is_possible_input[m_character] = true;
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTLiteral is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters
    ) -> void override {
        // Do nothing
    }

    /**
     * Add the needed Nfa::states to the passed in nfa to handle a
     * RegexASTLiteral before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_character() const -> uint32_t const& { return m_character; }

private:
    uint32_t m_character;
};

template <typename NfaStateType>
class RegexASTInteger : public RegexAST<NfaStateType> {
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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
        for (uint32_t const i : m_digits) {
            is_possible_input.at('0' + i) = true;
        }
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTInteger is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters
    ) -> void override {
        // Do nothing
    }

    /**
     * Add the needed Nfa::states to the passed in nfa to handle a
     * RegexASTInteger before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_digits() const -> std::vector<uint32_t> const& { return m_digits; }

    [[nodiscard]] auto get_digit(uint32_t i) const -> uint32_t const& { return m_digits[i]; }

private:
    std::vector<uint32_t> m_digits;
};

template <typename NfaStateType>
class RegexASTGroup : public RegexAST<NfaStateType> {
public:
    using Range = std::pair<uint32_t, uint32_t>;

    RegexASTGroup() = default;

    explicit RegexASTGroup(RegexASTLiteral<NfaStateType> const* right);

    explicit RegexASTGroup(RegexASTGroup const* right);

    RegexASTGroup(RegexASTGroup const* left, RegexASTLiteral<NfaStateType> const* right);

    RegexASTGroup(RegexASTGroup const* left, RegexASTGroup const* right);

    RegexASTGroup(
            RegexASTLiteral<NfaStateType> const* left,
            RegexASTLiteral<NfaStateType> const* right
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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
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

    /**
     * Add the needed Nfa::states to the passed in nfa to handle a
     * RegexASTGroup before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void override;

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

template <typename NfaStateType>
class RegexASTOr : public RegexAST<NfaStateType> {
public:
    ~RegexASTOr() override = default;

    RegexASTOr(
            std::unique_ptr<RegexAST<NfaStateType>> left,
            std::unique_ptr<RegexAST<NfaStateType>> right
    );

    RegexASTOr(RegexASTOr const& rhs)
            : RegexAST<NfaStateType>(rhs),
              m_left(std::unique_ptr<RegexAST<NfaStateType>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<NfaStateType>>(rhs.m_right->clone())) {}

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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
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

    /**
     * Add the needed Nfa::states to the passed in nfa to handle a
     * RegexASTOr before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_left() const -> RegexAST<NfaStateType> const* { return m_left.get(); }

    [[nodiscard]] auto get_right() const -> RegexAST<NfaStateType> const* { return m_right.get(); }

private:
    std::unique_ptr<RegexAST<NfaStateType>> m_left;
    std::unique_ptr<RegexAST<NfaStateType>> m_right;
};

template <typename NfaStateType>
class RegexASTCat : public RegexAST<NfaStateType> {
public:
    ~RegexASTCat() override = default;

    RegexASTCat(
            std::unique_ptr<RegexAST<NfaStateType>> left,
            std::unique_ptr<RegexAST<NfaStateType>> right
    );

    RegexASTCat(RegexASTCat const& rhs)
            : RegexAST<NfaStateType>(rhs),
              m_left(std::unique_ptr<RegexAST<NfaStateType>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<NfaStateType>>(rhs.m_right->clone())) {}

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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
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

    /**
     * Add the needed Nfa::states to the passed in nfa to handle a
     * RegexASTCat before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_left() const -> RegexAST<NfaStateType> const* { return m_left.get(); }

    [[nodiscard]] auto get_right() const -> RegexAST<NfaStateType> const* { return m_right.get(); }

private:
    std::unique_ptr<RegexAST<NfaStateType>> m_left;
    std::unique_ptr<RegexAST<NfaStateType>> m_right;
};

template <typename NfaStateType>
class RegexASTMultiplication : public RegexAST<NfaStateType> {
public:
    ~RegexASTMultiplication() override = default;

    RegexASTMultiplication(
            std::unique_ptr<RegexAST<NfaStateType>> operand,
            uint32_t min,
            uint32_t max
    );

    RegexASTMultiplication(RegexASTMultiplication const& rhs)
            : RegexAST<NfaStateType>(rhs),
              m_operand(std::unique_ptr<RegexAST<NfaStateType>>(rhs.m_operand->clone())),
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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
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

    /**
     * Add the needed Nfa::states to the passed in nfa to handle a
     * RegexASTMultiplication before transitioning to an accepting end_state
     * @param nfa
     * @param end_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto is_infinite() const -> bool { return m_max == 0; }

    [[nodiscard]] auto get_operand() const -> std::unique_ptr<RegexAST<NfaStateType>> const& {
        return m_operand;
    }

    [[nodiscard]] auto get_min() const -> uint32_t { return m_min; }

    [[nodiscard]] auto get_max() const -> uint32_t { return m_max; }

private:
    std::unique_ptr<RegexAST<NfaStateType>> m_operand;
    uint32_t m_min;
    uint32_t m_max;
};

/**
 * Represents a capture group AST node.
 * NOTE:
 * - `m_tag` is always expected to be non-null.
 * - `m_group_regex_ast` is always expected to be non-null.
 * @tparam NfaStateType Specifies the type of transition (bytes or UTF-8 characters).
 */
template <typename NfaStateType>
class RegexASTCapture : public RegexAST<NfaStateType> {
public:
    ~RegexASTCapture() override = default;

    /**
     * @param group_regex_ast
     * @param tag
     * @throw std::invalid_argument if `group_regex_ast` or `tag` are `nullptr`.
     */
    RegexASTCapture(
            std::unique_ptr<RegexAST<NfaStateType>> group_regex_ast,
            std::unique_ptr<Tag> tag
    )
            : m_group_regex_ast{(
                      nullptr == group_regex_ast
                              ? throw std::invalid_argument("Group regex AST cannot be null")
                              : std::move(group_regex_ast)
              )},
              m_tag{nullptr == tag ? throw std::invalid_argument("Tag cannot be null")
                                   : std::move(tag)} {
        RegexAST<NfaStateType>::set_subtree_positive_tags(
                m_group_regex_ast->get_subtree_positive_tags()
        );
        RegexAST<NfaStateType>::add_subtree_positive_tags({m_tag.get()});
    }

    RegexASTCapture(RegexASTCapture const& rhs)
            : RegexAST<NfaStateType>{rhs},
              m_group_regex_ast{
                      std::unique_ptr<RegexAST<NfaStateType>>(rhs.m_group_regex_ast->clone())
              },
              m_tag{std::make_unique<Tag>(*rhs.m_tag)} {
        RegexAST<NfaStateType>::set_subtree_positive_tags(rhs.get_subtree_positive_tags());
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
    auto set_possible_inputs_to_true(std::array<bool, cSizeOfUnicode>& is_possible_input
    ) const -> void override {
        m_group_regex_ast->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * `RegexASTGroup` with `.` is a descendant of this `RegexASTCapture` node.
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_group_regex_ast->remove_delimiters_from_wildcard(delimiters);
    }

    /**
     * Adds the needed `Nfa::states` to the passed in nfa to handle a
     * `RegexASTCapture` before transitioning to a `dest_state`.
     * @param nfa
     * @param dest_state
     */
    auto add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* dest_state) const -> void override;

    [[nodiscard]] auto serialize() const -> std::u32string override;

    [[nodiscard]] auto get_group_name() const -> std::string_view { return m_tag->get_name(); }

    [[nodiscard]] auto get_group_regex_ast(
    ) const -> std::unique_ptr<RegexAST<NfaStateType>> const& {
        return m_group_regex_ast;
    }

private:
    std::unique_ptr<RegexAST<NfaStateType>> m_group_regex_ast;
    std::unique_ptr<Tag> m_tag;
};

template <typename NfaStateType>
[[nodiscard]] auto RegexASTEmpty<NfaStateType>::serialize() const -> std::u32string {
    return fmt::format(U"{}", RegexAST<NfaStateType>::serialize_negative_tags());
}

template <typename NfaStateType>
RegexASTLiteral<NfaStateType>::RegexASTLiteral(uint32_t character) : m_character(character) {}

template <typename NfaStateType>
void RegexASTLiteral<NfaStateType>::add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state)
        const {
    nfa->add_root_interval(Interval(m_character, m_character), end_state);
}

template <typename NfaStateType>
[[nodiscard]] auto RegexASTLiteral<NfaStateType>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}",
            static_cast<char32_t>(m_character),
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}

template <typename NfaStateType>
RegexASTInteger<NfaStateType>::RegexASTInteger(uint32_t digit) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename NfaStateType>
RegexASTInteger<NfaStateType>::RegexASTInteger(RegexASTInteger* left, uint32_t digit)
        : m_digits(std::move(left->m_digits)) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename NfaStateType>
void RegexASTInteger<NfaStateType>::add_to_nfa(
        [[maybe_unused]] Nfa<NfaStateType>* nfa,
        [[maybe_unused]] NfaStateType* end_state
) const {
    throw std::runtime_error("Unsupported");
}

template <typename NfaStateType>
[[nodiscard]] auto RegexASTInteger<NfaStateType>::serialize() const -> std::u32string {
    auto const digits_string = fmt::format("{}", fmt::join(m_digits, ""));
    return fmt::format(
            U"{}{}",
            std::u32string(digits_string.begin(), digits_string.end()),
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}

template <typename NfaStateType>
RegexASTOr<NfaStateType>::RegexASTOr(
        std::unique_ptr<RegexAST<NfaStateType>> left,
        std::unique_ptr<RegexAST<NfaStateType>> right
)
        : m_left(std::move(left)),
          m_right(std::move(right)) {
    m_left->set_negative_tags(m_right->get_subtree_positive_tags());
    m_right->set_negative_tags(m_left->get_subtree_positive_tags());
    RegexAST<NfaStateType>::set_subtree_positive_tags(m_left->get_subtree_positive_tags());
    RegexAST<NfaStateType>::add_subtree_positive_tags(m_right->get_subtree_positive_tags());
}

template <typename NfaStateType>
void RegexASTOr<NfaStateType>::add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const {
    m_left->add_to_nfa_with_negative_tags(nfa, end_state);
    m_right->add_to_nfa_with_negative_tags(nfa, end_state);
}

template <typename NfaStateType>
[[nodiscard]] auto RegexASTOr<NfaStateType>::serialize() const -> std::u32string {
    return fmt::format(
            U"({})|({}){}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}

template <typename NfaStateType>
RegexASTCat<NfaStateType>::RegexASTCat(
        std::unique_ptr<RegexAST<NfaStateType>> left,
        std::unique_ptr<RegexAST<NfaStateType>> right
)
        : m_left(std::move(left)),
          m_right(std::move(right)) {
    RegexAST<NfaStateType>::set_subtree_positive_tags(m_left->get_subtree_positive_tags());
    RegexAST<NfaStateType>::add_subtree_positive_tags(m_right->get_subtree_positive_tags());
}

template <typename NfaStateType>
void RegexASTCat<NfaStateType>::add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state) const {
    NfaStateType* saved_root = nfa->get_root();
    NfaStateType* intermediate_state = nfa->new_state();
    m_left->add_to_nfa_with_negative_tags(nfa, intermediate_state);
    nfa->set_root(intermediate_state);
    m_right->add_to_nfa_with_negative_tags(nfa, end_state);
    nfa->set_root(saved_root);
}

template <typename NfaStateType>
[[nodiscard]] auto RegexASTCat<NfaStateType>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}{}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}

template <typename NfaStateType>
RegexASTMultiplication<NfaStateType>::RegexASTMultiplication(
        std::unique_ptr<RegexAST<NfaStateType>> operand,
        uint32_t const min,
        uint32_t const max
)
        : m_operand(std::move(operand)),
          m_min(min),
          m_max(max) {
    RegexAST<NfaStateType>::set_subtree_positive_tags(m_operand->get_subtree_positive_tags());
}

template <typename NfaStateType>
void RegexASTMultiplication<NfaStateType>::add_to_nfa(
        Nfa<NfaStateType>* nfa,
        NfaStateType* end_state
) const {
    NfaStateType* saved_root = nfa->get_root();
    if (0 == m_min) {
        nfa->get_root()->add_epsilon_transition(end_state);
    } else {
        for (uint32_t i = 1; i < m_min; i++) {
            NfaStateType* intermediate_state = nfa->new_state();
            m_operand->add_to_nfa_with_negative_tags(nfa, intermediate_state);
            nfa->set_root(intermediate_state);
        }
        m_operand->add_to_nfa_with_negative_tags(nfa, end_state);
    }
    if (is_infinite()) {
        nfa->set_root(end_state);
        m_operand->add_to_nfa_with_negative_tags(nfa, end_state);
    } else if (m_max > m_min) {
        if (0 != m_min) {
            NfaStateType* intermediate_state = nfa->new_state();
            m_operand->add_to_nfa_with_negative_tags(nfa, intermediate_state);
            nfa->set_root(intermediate_state);
        }
        for (uint32_t i = m_min + 1; i < m_max; ++i) {
            m_operand->add_to_nfa_with_negative_tags(nfa, end_state);
            NfaStateType* intermediate_state = nfa->new_state();
            m_operand->add_to_nfa_with_negative_tags(nfa, intermediate_state);
            nfa->set_root(intermediate_state);
        }
        m_operand->add_to_nfa_with_negative_tags(nfa, end_state);
    }
    nfa->set_root(saved_root);
}

template <typename NfaStateType>
[[nodiscard]] auto RegexASTMultiplication<NfaStateType>::serialize() const -> std::u32string {
    auto const min_string = std::to_string(m_min);
    auto const max_string = std::to_string(m_max);

    return fmt::format(
            U"({}){{{},{}}}{}",
            nullptr != m_operand ? m_operand->serialize() : U"null",
            std::u32string(min_string.begin(), min_string.end()),
            is_infinite() ? U"inf" : std::u32string(max_string.begin(), max_string.end()),
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}

template <typename NfaStateType>
auto RegexASTCapture<NfaStateType>::add_to_nfa(
        Nfa<NfaStateType>* nfa,
        NfaStateType* dest_state
) const -> void {
    // TODO: move this into a documentation file in the future, and reference it here.
    // The NFA constructed for a capture group follows the structure below, with tagged transitions
    // explicitly labeled for clarity:
    //         +---------------------+
    //         |       `m_root`      |
    //         +---------------------+
    //                    | `m_tag` start
    //                    | (positive tagged start transition)
    //                    v
    //         +---------------------+
    //         |`capture_start_state`|
    //         +---------------------+
    //                    |
    //                    | (epsilon transition)
    //                    v
    //         +---------------------+
    //         | `m_group_regex_ast` |
    //         |    (nested NFA)     |
    //         +---------------------+
    //                    | `m_negative_tags`
    //                    | (negative tagged transition)
    //                    v
    //         +---------------------+
    //         | `capture_end_state` |
    //         +---------------------+
    //                    | `m_tag` end
    //                    | (positive tagged end transition)
    //                    v
    //         +---------------------+
    //         |     `dest_state`    |
    //         +---------------------+
    auto [capture_start_state, capture_end_state]
            = nfa->new_start_and_end_states_with_positive_tagged_transitions(
                    m_tag.get(),
                    dest_state
            );

    auto* initial_root = nfa->get_root();
    nfa->set_root(capture_start_state);
    m_group_regex_ast->add_to_nfa_with_negative_tags(nfa, capture_end_state);
    nfa->set_root(initial_root);
}

template <typename NfaStateType>
[[nodiscard]] auto RegexASTCapture<NfaStateType>::serialize() const -> std::u32string {
    auto const tag_name_u32 = std::u32string(m_tag->get_name().cbegin(), m_tag->get_name().cend());
    return fmt::format(
            U"({})<{}>{}",
            m_group_regex_ast->serialize(),
            tag_name_u32,
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(
        RegexASTGroup const* left,
        RegexASTLiteral<NfaStateType> const* right
) {
    if (right == nullptr) {
        throw std::runtime_error("RegexASTGroup1: right == nullptr: A bracket expression in the "
                                 "schema contains illegal characters, remember to escape special "
                                 "characters. Refer to README-Schema.md for more details.");
    }
    m_negate = left->m_negate;
    m_ranges = left->m_ranges;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(RegexASTGroup const* left, RegexASTGroup const* right)
        : m_negate(left->m_negate),
          m_ranges(left->m_ranges) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(RegexASTLiteral<NfaStateType> const* right) {
    if (right == nullptr) {
        throw std::runtime_error("RegexASTGroup2: right == nullptr: A bracket expression in the "
                                 "schema contains illegal characters, remember to escape special "
                                 "characters. Refer to README-Schema.md for more details.");
    }
    m_negate = false;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(RegexASTGroup const* right) : m_negate(false) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(
        RegexASTLiteral<NfaStateType> const* left,
        RegexASTLiteral<NfaStateType> const* right
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

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(std::vector<uint32_t> const& literals)
        : m_negate(false) {
    for (uint32_t literal : literals) {
        m_ranges.emplace_back(literal, literal);
    }
}

template <typename NfaStateType>
RegexASTGroup<NfaStateType>::RegexASTGroup(uint32_t min, uint32_t max) : m_negate(false) {
    m_ranges.emplace_back(min, max);
}

// ranges must be sorted
template <typename NfaStateType>
auto RegexASTGroup<NfaStateType>::merge(std::vector<Range> const& ranges) -> std::vector<Range> {
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
template <typename NfaStateType>
auto RegexASTGroup<NfaStateType>::complement(std::vector<Range> const& ranges
) -> std::vector<Range> {
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

template <typename NfaStateType>
void RegexASTGroup<NfaStateType>::add_to_nfa(Nfa<NfaStateType>* nfa, NfaStateType* end_state)
        const {
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

template <typename NfaStateType>
[[nodiscard]] auto RegexASTGroup<NfaStateType>::serialize() const -> std::u32string {
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
            RegexAST<NfaStateType>::serialize_negative_tags()
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
