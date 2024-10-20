#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP

#include <algorithm>
#include <array>
#include <cassert>
#include <cstdint>
#include <map>
#include <memory>
#include <set>
#include <stack>
#include <tuple>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>
#include <log_surgeon/LexicalRule.hpp>

namespace log_surgeon::finite_automata {
enum class RegexNFAStateType : uint8_t {
    Byte,
    UTF8
};

template <RegexNFAStateType state_type>
class RegexNFAState;

template <RegexNFAStateType state_type>
class PositiveTaggedTransition {
public:
    PositiveTaggedTransition(uint32_t const tag, RegexNFAState<state_type> const* dest_state)
            : m_tag(tag),
              m_dest_state(dest_state) {}

    [[nodiscard]] auto get_tag() const -> uint32_t { return m_tag; }

    [[nodiscard]] auto get_dest_state() const -> RegexNFAState<state_type> const* {
        return m_dest_state;
    }

private:
    uint32_t m_tag{};
    RegexNFAState<state_type> const* m_dest_state{};
};

template <RegexNFAStateType state_type>
class NegativeTaggedTransition {
public:
    NegativeTaggedTransition(std::set<uint32_t> tags, RegexNFAState<state_type> const* dest_state)
            : m_tags(std::move(tags)),
              m_dest_state(dest_state) {}

    [[nodiscard]] auto get_tags() const -> std::set<uint32_t> const& { return m_tags; }

    [[nodiscard]] auto get_dest_state() const -> RegexNFAState<state_type> const* {
        return m_dest_state;
    }

private:
    std::set<uint32_t> m_tags;
    RegexNFAState<state_type> const* m_dest_state{};
};

template <RegexNFAStateType state_type>
class RegexNFAState {
public:
    RegexNFAState() = default;

    explicit RegexNFAState(uint32_t const tag, RegexNFAState const* dest_state) {
        m_positive_tagged_transitions.emplace_back(tag, dest_state);
    }

    explicit RegexNFAState(std::set<uint32_t> tags, RegexNFAState const* dest_state) {
        m_negative_tagged_transitions.emplace_back(std::move(tags), dest_state);
    }

    using Tree = UnicodeIntervalTree<RegexNFAState*>;

    auto set_accepting(bool accepting) -> void { m_accepting = accepting; }

    [[nodiscard]] auto is_accepting() const -> bool const& { return m_accepting; }

    auto set_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_id = variable_id;
    }

    [[nodiscard]] auto get_matching_variable_id() const -> uint32_t {
        return m_matching_variable_id;
    }

    [[nodiscard]] auto get_positive_tagged_transitions(
    ) const -> std::vector<PositiveTaggedTransition<state_type>> const& {
        return m_positive_tagged_transitions;
    }

    [[nodiscard]] auto get_negative_tagged_transitions(
    ) const -> std::vector<NegativeTaggedTransition<state_type>> const& {
        return m_negative_tagged_transitions;
    }

    auto add_epsilon_transition(RegexNFAState* epsilon_transition) -> void {
        m_epsilon_transitions.push_back(epsilon_transition);
    }

    [[nodiscard]] auto get_epsilon_transitions() const -> std::vector<RegexNFAState*> const& {
        return m_epsilon_transitions;
    }

    auto add_byte_transition(uint8_t byte, RegexNFAState* dest_state) -> void {
        m_bytes_transitions[byte].push_back(dest_state);
    }

    [[nodiscard]] auto get_byte_transitions(uint8_t byte
    ) const -> std::vector<RegexNFAState*> const& {
        return m_bytes_transitions[byte];
    }

    auto get_tree_transitions() -> Tree const& { return m_tree_transitions; }

    /**
     Add dest_state to m_bytes_transitions if all values in interval are a byte, otherwise add
     dest_state to m_tree_transitions
     * @param interval
     * @param dest_state
     */
    auto add_interval(Interval interval, RegexNFAState* dest_state) -> void;

private:
    bool m_accepting{false};
    uint32_t m_matching_variable_id{0};
    std::vector<PositiveTaggedTransition<state_type>> m_positive_tagged_transitions;
    std::vector<NegativeTaggedTransition<state_type>> m_negative_tagged_transitions;
    std::vector<RegexNFAState*> m_epsilon_transitions;
    std::array<std::vector<RegexNFAState*>, cSizeOfByte> m_bytes_transitions;
    // NOTE: We don't need m_tree_transitions for the `stateType ==
    // RegexDFAStateType::Byte` case, so we use an empty class (`std::tuple<>`)
    // in that case.
    std::conditional_t<state_type == RegexNFAStateType::UTF8, Tree, std::tuple<>>
            m_tree_transitions;
};

using RegexNFAByteState = RegexNFAState<RegexNFAStateType::Byte>;
using RegexNFAUTF8State = RegexNFAState<RegexNFAStateType::UTF8>;

// TODO: rename `RegexNFA` to `NFA`
template <typename NFAStateType>
class RegexNFA {
public:
    using StateVec = std::vector<NFAStateType*>;

    explicit RegexNFA(std::vector<LexicalRule<NFAStateType>> const& m_rules);

    /**
     * Create a unique_ptr for an NFA state with no tagged transitions and add it to m_states.
     * @return NFAStateType*
     */
    auto new_state() -> NFAStateType*;

    /**
     * Create a unique_ptr for an NFA state with a positive tagged transition and add it to
     * m_states.
     * @return NFAStateType*
     */
    auto new_state_with_a_positive_tagged_transition(uint32_t tag, NFAStateType const* dest_state)
            -> NFAStateType*;

    /**
     * Create a unique_ptr for an NFA state with negative tagged transitions and add it to m_states.
     * @return NFAStateType*
     */
    auto new_state_with_negative_tagged_transitions(
            std::set<uint32_t> tags,
            NFAStateType const* dest_state
    ) -> NFAStateType*;

    /**
     * Reverse the NFA such that it matches on its reverse language
     */
    auto reverse() -> void;

    auto add_root_interval(Interval interval, NFAStateType* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(NFAStateType* root) -> void { m_root = root; }

    auto get_root() -> NFAStateType* { return m_root; }

private:
    std::vector<std::unique_ptr<NFAStateType>> m_states;
    NFAStateType* m_root;
};

template <RegexNFAStateType state_type>
void RegexNFAState<state_type>::add_interval(Interval interval, RegexNFAState* dest_state) {
    if (interval.first < cSizeOfByte) {
        uint32_t const bound = std::min(interval.second, cSizeOfByte - 1);
        for (uint32_t i = interval.first; i <= bound; i++) {
            add_byte_transition(i, dest_state);
        }
        interval.first = bound + 1;
    }
    if constexpr (RegexNFAStateType::UTF8 == state_type) {
        if (interval.second < cSizeOfByte) {
            return;
        }
        std::unique_ptr<std::vector<typename Tree::Data>> overlaps
                = m_tree_transitions.pop(interval);
        for (typename Tree::Data const& data : *overlaps) {
            uint32_t overlap_low = std::max(data.m_interval.first, interval.first);
            uint32_t overlap_high = std::min(data.m_interval.second, interval.second);

            std::vector<RegexNFAUTF8State*> tree_states = data.m_value;
            tree_states.push_back(dest_state);
            m_tree_transitions.insert(Interval(overlap_low, overlap_high), tree_states);
            if (data.m_interval.first < interval.first) {
                m_tree_transitions.insert(
                        Interval(data.m_interval.first, interval.first - 1),
                        data.m_value
                );
            } else if (data.m_interval.first > interval.first) {
                m_tree_transitions.insert(
                        Interval(interval.first, data.m_interval.first - 1),
                        {dest_state}
                );
            }
            if (data.m_interval.second > interval.second) {
                m_tree_transitions.insert(
                        Interval(interval.second + 1, data.m_interval.second),
                        data.m_value
                );
            }
            interval.first = data.m_interval.second + 1;
        }
        if (interval.first != 0 && interval.first <= interval.second) {
            m_tree_transitions.insert(interval, {dest_state});
        }
    }
}

template <typename NFAStateType>
RegexNFA<NFAStateType>::RegexNFA(std::vector<LexicalRule<NFAStateType>> const& m_rules)
        : m_root{new_state()} {
    for (auto const& rule : m_rules) {
        rule.add_to_nfa(this);
    }
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_state() -> NFAStateType* {
    std::unique_ptr<NFAStateType> ptr = std::make_unique<NFAStateType>();
    NFAStateType* state = ptr.get();
    m_states.push_back(std::move(ptr));
    return state;
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_state_with_a_positive_tagged_transition(
        uint32_t const tag,
        NFAStateType const* dest_state
) -> NFAStateType* {
    std::unique_ptr<NFAStateType> ptr = std::make_unique<NFAStateType>(tag, dest_state);
    NFAStateType* state = ptr.get();
    m_states.push_back(std::move(ptr));
    return state;
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_state_with_negative_tagged_transitions(
        std::set<uint32_t> tags,
        NFAStateType const* dest_state
) -> NFAStateType* {
    std::unique_ptr<NFAStateType> ptr = std::make_unique<NFAStateType>(tags, dest_state);
    NFAStateType* state = ptr.get();
    m_states.push_back(std::move(ptr));
    return state;
}

}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
