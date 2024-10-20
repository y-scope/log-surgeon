#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP

#include <algorithm>
#include <array>
#include <cassert>
#include <cstdint>
#include <memory>
#include <queue>
#include <stack>
#include <tuple>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

#include <fmt/core.h>

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

using RegexNFAByteState = RegexNFAState<RegexNFAStateType::Byte>;
using RegexNFAUTF8State = RegexNFAState<RegexNFAStateType::UTF8>;

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

    /**
     * Serialize the positive tagged transition into a string.
     */
    [[nodiscard]] auto serialize(
            std::unordered_map<RegexNFAByteState const*, uint32_t> const& state_ids
    ) const -> std::string;

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

    /**
     * Serialize the negative tagged transitions into a string.
     */
    [[nodiscard]] auto serialize(
            std::unordered_map<RegexNFAByteState const*, uint32_t> const& state_ids
    ) const -> std::string;

private:
    std::set<uint32_t> m_tags;
    RegexNFAState<state_type> const* m_dest_state{};
};

template <RegexNFAStateType state_type>
class RegexNFAState {
public:
    using Tree = UnicodeIntervalTree<RegexNFAState*>;

    RegexNFAState() = default;

    explicit RegexNFAState(uint32_t const tag, RegexNFAState const* dest_state) {
        m_positive_tagged_transitions.emplace_back(tag, dest_state);
    }

    explicit RegexNFAState(std::set<uint32_t> tags, RegexNFAState const* dest_state) {
        m_negative_tagged_transitions.emplace_back(std::move(tags), dest_state);
    }

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

    /**
     * Serialize the NFA state into a string.
     */
    [[nodiscard]] auto serialize(
            std::unordered_map<RegexNFAByteState const*, uint32_t> const& state_ids
    ) const -> std::string;

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
     * Serialize the NFA into a string.
     */
    [[nodiscard]] auto serialize() const -> std::string;

    auto add_root_interval(Interval interval, NFAStateType* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(NFAStateType* root) -> void { m_root = root; }

    auto get_root() -> NFAStateType* { return m_root; }

private:
    /**
     * Add a destination state to the queue and set of visited states if it has not yet been
     * visited.
     * @param dest_state
     * @param visited_states
     * @param state_queue
     */
    static auto add_to_queue_and_visited(
            RegexNFAByteState const* dest_state,
            std::queue<RegexNFAByteState const*>& state_queue,
            std::unordered_set<RegexNFAByteState const*>& visited_states
    ) -> void;

    std::vector<std::unique_ptr<NFAStateType>> m_states;
    NFAStateType* m_root;
};

template <RegexNFAStateType state_type>
auto PositiveTaggedTransition<state_type>::serialize(
        std::unordered_map<RegexNFAByteState const*, uint32_t> const& state_ids
) const -> std::string {
    return fmt::format("{}[{}]", state_ids.at(get_dest_state()), get_tag());
}

template <RegexNFAStateType state_type>
auto NegativeTaggedTransition<state_type>::serialize(
        std::unordered_map<RegexNFAByteState const*, uint32_t> const& state_ids
) const -> std::string {
    return fmt::format("{}[{}]", state_ids.at(get_dest_state()), fmt::join(get_tags(), ","));
}

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

template <RegexNFAStateType state_type>
auto RegexNFAState<state_type>::serialize(
        std::unordered_map<RegexNFAByteState const*, uint32_t> const& state_ids
) const -> std::string {
    std::vector<std::string> byte_transitions;
    for (uint32_t idx = 0; idx < cSizeOfByte; idx++) {
        for (auto const* dest_state : m_bytes_transitions[idx]) {
            byte_transitions.push_back(
                    fmt::format("{}-->{}", static_cast<char>(idx), state_ids.at(dest_state))
            );
        }
    }
    std::vector<std::string> epsilon_transitions;
    for (auto const* dest_state : m_epsilon_transitions) {
        epsilon_transitions.push_back(std::to_string(state_ids.at(dest_state)));
    }
    std::vector<std::string> positive_tagged_transitions;
    for (auto const& positive_tagged_transition : m_positive_tagged_transitions) {
        positive_tagged_transitions.push_back(positive_tagged_transition.serialize(state_ids));
    }
    std::vector<std::string> negative_tagged_transitions;
    for (auto const& negative_tagged_transition : m_negative_tagged_transitions) {
        negative_tagged_transitions.push_back(negative_tagged_transition.serialize(state_ids));
    }

    auto accepting_tag_string
            = m_accepting ? fmt::format("accepting_tag={},", m_matching_variable_id) : "";

    return fmt::format(
            "{}:{}byte_transitions={{{}}},epsilon_transitions={{{}}},positive_tagged_transitions={{"
            "{}}},negative_tagged_transitions={{{}}}",
            state_ids.at(this),
            accepting_tag_string,
            fmt::join(byte_transitions, ","),
            fmt::join(epsilon_transitions, ","),
            fmt::join(positive_tagged_transitions, ","),
            fmt::join(negative_tagged_transitions, ",")
    );
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

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::add_to_queue_and_visited(
        RegexNFAByteState const* dest_state,
        std::queue<RegexNFAByteState const*>& state_queue,
        std::unordered_set<RegexNFAByteState const*>& visited_states
) -> void {
    if (visited_states.insert(dest_state).second) {
        state_queue.push(dest_state);
    }
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::serialize() const -> std::string {
    std::queue<RegexNFAByteState const*> state_queue;
    std::queue<RegexNFAByteState const*> state_queue_copy;
    std::unordered_set<RegexNFAByteState const*> visited_states;

    // Assign state IDs
    std::unordered_map<RegexNFAByteState const*, uint32_t> state_ids;
    add_to_queue_and_visited(m_root, state_queue, visited_states);
    while (false == state_queue.empty()) {
        auto const* current_state = state_queue.front();
        state_queue_copy.push(current_state);
        state_queue.pop();
        state_ids.insert({current_state, state_ids.size()});
        for (uint32_t idx = 0; idx < cSizeOfByte; idx++) {
            for (auto const* dest_state : current_state->get_byte_transitions(idx)) {
                add_to_queue_and_visited(dest_state, state_queue, visited_states);
            }
        }
        for (auto const* dest_state : current_state->get_epsilon_transitions()) {
            add_to_queue_and_visited(dest_state, state_queue, visited_states);
        }
        for (auto const& positive_tagged_transition :
             current_state->get_positive_tagged_transitions())
        {
            add_to_queue_and_visited(
                    positive_tagged_transition.get_dest_state(),
                    state_queue,
                    visited_states
            );
        }
        for (auto const& negative_tagged_transition :
             current_state->get_negative_tagged_transitions())
        {
            add_to_queue_and_visited(
                    negative_tagged_transition.get_dest_state(),
                    state_queue,
                    visited_states
            );
        }
    }

    // Serialize NFA
    std::vector<std::string> serialized_states;
    while (false == state_queue_copy.empty()) {
        auto const* current_state = state_queue_copy.front();
        state_queue_copy.pop();
        serialized_states.emplace_back(current_state->serialize(state_ids));
    }

    return fmt::format("{}\n", fmt::join(serialized_states, "\n"));
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
