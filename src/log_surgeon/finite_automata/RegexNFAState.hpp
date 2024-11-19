#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_STATE
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_STATE

#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <tuple>
#include <unordered_map>
#include <vector>

#include <fmt/format.h>

#include <log_surgeon/finite_automata/RegexNFAStateType.hpp>
#include <log_surgeon/finite_automata/TaggedTransition.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
template <RegexNFAStateType state_type>
class RegexNFAState;

using RegexNFAByteState = RegexNFAState<RegexNFAStateType::Byte>;
using RegexNFAUTF8State = RegexNFAState<RegexNFAStateType::UTF8>;

template <RegexNFAStateType state_type>
class RegexNFAState {
public:
    using Tree = UnicodeIntervalTree<RegexNFAState*>;

    RegexNFAState() = default;

    RegexNFAState(Tag const* tag, RegexNFAState const* dest_state)
            : m_positive_tagged_end_transition{PositiveTaggedTransition{tag, dest_state}} {}

    RegexNFAState(std::vector<Tag const*> tags, RegexNFAState const* dest_state)
            : m_negative_tagged_transition{NegativeTaggedTransition{std::move(tags), dest_state}} {}

    auto set_accepting(bool accepting) -> void { m_accepting = accepting; }

    [[nodiscard]] auto is_accepting() const -> bool const& { return m_accepting; }

    auto set_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_id = variable_id;
    }

    [[nodiscard]] auto get_matching_variable_id() const -> uint32_t {
        return m_matching_variable_id;
    }

    auto
    add_positive_tagged_start_transition(Tag const* tag, RegexNFAState const* dest_state) -> void {
        m_positive_tagged_start_transitions.emplace_back(tag, dest_state);
    }

    [[nodiscard]] auto get_positive_tagged_start_transitions(
    ) const -> std::vector<PositiveTaggedTransition<RegexNFAState>> const& {
        return m_positive_tagged_start_transitions;
    }

    [[nodiscard]] auto get_positive_tagged_end_transitions(
    ) const -> std::optional<PositiveTaggedTransition<RegexNFAState>> const& {
        return m_positive_tagged_end_transition;
    }

    [[nodiscard]] auto get_negative_tagged_transition(
    ) const -> std::optional<NegativeTaggedTransition<RegexNFAState>> const& {
        return m_negative_tagged_transition;
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
     * @param state_ids A map of states to their unique identifiers.
     * @return A string representation of the NFA state on success.
     * @return Forwards `PositiveTaggedTransition::serialize`'s return value (std::nullopt) on
     * failure.
     * @return Forwards `NegativeTaggedTransition::serialize`'s return value (std::nullopt) on
     * failure.
     */
    [[nodiscard]] auto serialize(std::unordered_map<RegexNFAState const*, uint32_t> const& state_ids
    ) const -> std::optional<std::string>;

private:
    bool m_accepting{false};
    uint32_t m_matching_variable_id{0};
    std::vector<PositiveTaggedTransition<RegexNFAState>> m_positive_tagged_start_transitions;
    std::optional<PositiveTaggedTransition<RegexNFAState>> m_positive_tagged_end_transition;
    std::optional<NegativeTaggedTransition<RegexNFAState>> m_negative_tagged_transition;
    std::vector<RegexNFAState*> m_epsilon_transitions;
    std::array<std::vector<RegexNFAState*>, cSizeOfByte> m_bytes_transitions;
    // NOTE: We don't need m_tree_transitions for the `stateType ==
    // RegexDFAStateType::Byte` case, so we use an empty class (`std::tuple<>`)
    // in that case.
    std::conditional_t<state_type == RegexNFAStateType::UTF8, Tree, std::tuple<>>
            m_tree_transitions;
};

template <RegexNFAStateType state_type>
auto RegexNFAState<state_type>::add_interval(Interval interval, RegexNFAState* dest_state) -> void {
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
        std::unordered_map<RegexNFAState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    std::vector<std::string> byte_transitions;
    for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
        for (auto const* dest_state : m_bytes_transitions[idx]) {
            byte_transitions.emplace_back(
                    fmt::format("{}-->{}", static_cast<char>(idx), state_ids.at(dest_state))
            );
        }
    }

    std::vector<std::string> epsilon_transitions;
    for (auto const* dest_state : m_epsilon_transitions) {
        epsilon_transitions.emplace_back(std::to_string(state_ids.at(dest_state)));
    }

    std::vector<std::string> positive_tagged_start_transition_strings;
    for (auto const& positive_tagged_start_transition : m_positive_tagged_start_transitions) {
        auto const optional_serialized_positive_start_transition
                = positive_tagged_start_transition.serialize(state_ids);
        if (false == optional_serialized_positive_start_transition.has_value()) {
            return std::nullopt;
        }
        positive_tagged_start_transition_strings.emplace_back(
                optional_serialized_positive_start_transition.value()
        );
    }

    std::string positive_tagged_end_transition_string;
    if (m_positive_tagged_end_transition.has_value()) {
        auto const optional_serialized_positive_end_transition
                = m_positive_tagged_end_transition.value().serialize(state_ids);
        if (false == optional_serialized_positive_end_transition.has_value()) {
            return std::nullopt;
        }
        positive_tagged_end_transition_string = optional_serialized_positive_end_transition.value();
    }

    std::string negative_tagged_transition_string;
    if (m_negative_tagged_transition.has_value()) {
        auto const optional_serialized_negative_transition
                = m_negative_tagged_transition.value().serialize(state_ids);
        if (false == optional_serialized_negative_transition.has_value()) {
            return std::nullopt;
        }
        negative_tagged_transition_string = optional_serialized_negative_transition.value();
    }

    auto const accepting_tag_string
            = m_accepting ? fmt::format("accepting_tag={},", m_matching_variable_id) : "";

    return fmt::format(
            "{}:{}byte_transitions={{{}}},epsilon_transitions={{{}}},positive_tagged_start_"
            "transitions={{{}}},positive_tagged_end_transitions={{{}}},negative_tagged_transition={"
            "{{}}}",
            state_ids.at(this),
            accepting_tag_string,
            fmt::join(byte_transitions, ","),
            fmt::join(epsilon_transitions, ","),
            fmt::join(positive_tagged_start_transition_strings, ","),
            positive_tagged_end_transition_string,
            negative_tagged_transition_string
    );
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_STATE
