#ifndef LOG_SURGEON_FINITE_AUTOMATA_NFA_STATE
#define LOG_SURGEON_FINITE_AUTOMATA_NFA_STATE

#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <stack>
#include <string>
#include <tuple>
#include <unordered_map>
#include <vector>

#include <fmt/format.h>

#include <log_surgeon/finite_automata/SpontaneousTransition.hpp>
#include <log_surgeon/finite_automata/StateType.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {

template <StateType state_type>
class NfaState;

using ByteNfaState = NfaState<StateType::Byte>;
using Utf8NfaState = NfaState<StateType::Utf8>;

template <StateType state_type>
class NfaState {
public:
    using Tree = UnicodeIntervalTree<NfaState*>;

    NfaState() = default;

    NfaState(
            TransitionOperation transition_op,
            std::vector<tag_id_t> tag_ids,
            NfaState const* dest_state
    )
            : m_spontaneous_transitions{{transition_op, std::move(tag_ids), dest_state}} {}

    auto set_accepting(bool accepting) -> void { m_accepting = accepting; }

    auto set_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_id = variable_id;
    }

    auto add_spontaneous_transition(NfaState* dest_state) -> void {
        m_spontaneous_transitions.emplace_back(dest_state);
    }

    auto add_spontaneous_transition(
            TransitionOperation const transition_op,
            std::vector<tag_id_t> tag_ids,
            NfaState const* dest_state
    ) -> void {
        m_spontaneous_transitions.emplace_back(transition_op, std::move(tag_ids), dest_state);
    }

    auto add_byte_transition(uint8_t byte, NfaState* dest_state) -> void {
        m_bytes_transitions[byte].push_back(dest_state);
    }

    /**
     * Add `dest_state` to `m_bytes_transitions` if all values in interval are a byte, otherwise add
     * `dest_state` to `m_tree_transitions`.
     * @param interval
     * @param dest_state
     */
    auto add_interval(Interval interval, NfaState* dest_state) -> void;

    /**
     * @return The set of all states reachable from the current state via epsilon transitions.
     */
    auto epsilon_closure() -> std::set<NfaState const*>;

    /**
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the NFA state on success.
     * @return Forwards `SpontaneousTransition::serialize`'s return value (std::nullopt) on failure.
     */
    [[nodiscard]] auto serialize(std::unordered_map<NfaState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

    [[nodiscard]] auto is_accepting() const -> bool const& { return m_accepting; }

    [[nodiscard]] auto get_matching_variable_id() const -> uint32_t {
        return m_matching_variable_id;
    }

    [[nodiscard]] auto get_spontaneous_transitions(
    ) const -> std::vector<SpontaneousTransition<NfaState>> const& {
        return m_spontaneous_transitions;
    }

    [[nodiscard]] auto get_byte_transitions(uint8_t byte) const -> std::vector<NfaState*> const& {
        return m_bytes_transitions[byte];
    }

    [[nodiscard]] auto get_tree_transitions() -> Tree const& { return m_tree_transitions; }

private:
    bool m_accepting{false};
    uint32_t m_matching_variable_id{0};
    std::vector<SpontaneousTransition<NfaState>> m_spontaneous_transitions;
    std::array<std::vector<NfaState*>, cSizeOfByte> m_bytes_transitions;
    // NOTE: We don't need m_tree_transitions for the `stateType ==
    // StateType::Byte` case, so we use an empty class (`std::tuple<>`)
    // in that case.
    std::conditional_t<state_type == StateType::Utf8, Tree, std::tuple<>> m_tree_transitions;
};

template <StateType state_type>
auto NfaState<state_type>::add_interval(Interval interval, NfaState* dest_state) -> void {
    if (interval.first < cSizeOfByte) {
        uint32_t const bound = std::min(interval.second, cSizeOfByte - 1);
        for (uint32_t i = interval.first; i <= bound; i++) {
            add_byte_transition(i, dest_state);
        }
        interval.first = bound + 1;
    }
    if constexpr (StateType::Utf8 == state_type) {
        if (interval.second < cSizeOfByte) {
            return;
        }
        std::unique_ptr<std::vector<typename Tree::Data>> overlaps
                = m_tree_transitions.pop(interval);
        for (typename Tree::Data const& data : *overlaps) {
            uint32_t overlap_low = std::max(data.m_interval.first, interval.first);
            uint32_t overlap_high = std::min(data.m_interval.second, interval.second);

            std::vector<Utf8NfaState*> tree_states = data.m_value;
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

template <StateType state_type>
auto NfaState<state_type>::epsilon_closure() -> std::set<NfaState const*> {
    std::set<NfaState const*> closure_set;
    std::stack<NfaState const*> stack;
    stack.push(this);
    while (false == stack.empty()) {
        auto const* current_state = stack.top();
        stack.pop();
        if (false == closure_set.insert(current_state).second) {
            continue;
        }
        for (auto const& spontaneous_transition : current_state->get_spontaneous_transitions()) {
            stack.push(spontaneous_transition.get_dest_state());
        }
    }
    return closure_set;
}

template <StateType state_type>
auto NfaState<state_type>::serialize(std::unordered_map<NfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    auto const accepting_tag_string
            = m_accepting ? fmt::format("accepting_tag={},", m_matching_variable_id) : "";

    std::vector<std::string> byte_transitions;
    for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
        for (auto const* dest_state : m_bytes_transitions[idx]) {
            byte_transitions.emplace_back(
                    fmt::format("{}-->{}", static_cast<char>(idx), state_ids.at(dest_state))
            );
        }
    }

    std::vector<std::string> serialized_spontaneous_transitions;
    for (auto const& spontaneous_transition : m_spontaneous_transitions) {
        auto const optional_serialized_transition = spontaneous_transition.serialize(state_ids);
        if (false == optional_serialized_transition.has_value()) {
            return std::nullopt;
        }
        serialized_spontaneous_transitions.emplace_back(optional_serialized_transition.value());
    }

    return fmt::format(
            "{}:{}byte_transitions={{{}}},spontaneous_transition={{{}}}",
            state_ids.at(this),
            accepting_tag_string,
            fmt::join(byte_transitions, ","),
            fmt::join(serialized_spontaneous_transitions, ",")
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_NFA_STATE
