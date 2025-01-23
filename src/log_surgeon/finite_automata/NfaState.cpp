#include "NfaState.hpp"

#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <StateType state_type>
auto NfaState<state_type>::serialize(std::unordered_map<NfaState const*, uint32_t> const& state_ids
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

    std::vector<std::string> serialized_positive_tagged_start_transitions;
    for (auto const& positive_tagged_start_transition : m_positive_tagged_start_transitions) {
        auto const optional_serialized_positive_start_transition
                = positive_tagged_start_transition.serialize(state_ids);
        if (false == optional_serialized_positive_start_transition.has_value()) {
            return std::nullopt;
        }
        serialized_positive_tagged_start_transitions.emplace_back(
                optional_serialized_positive_start_transition.value()
        );
    }

    std::string serialized_positive_tagged_end_transition;
    if (m_positive_tagged_end_transition.has_value()) {
        auto const optional_serialized_positive_end_transition
                = m_positive_tagged_end_transition.value().serialize(state_ids);
        if (false == optional_serialized_positive_end_transition.has_value()) {
            return std::nullopt;
        }
        serialized_positive_tagged_end_transition
                = optional_serialized_positive_end_transition.value();
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
            fmt::join(serialized_positive_tagged_start_transitions, ","),
            serialized_positive_tagged_end_transition,
            negative_tagged_transition_string
    );
}

// Explicit template instantiations
template auto NfaState<StateType::Byte>::serialize(std::unordered_map<NfaState const*, uint32_t> const& state_ids)
    const -> std::optional<std::string>;
template auto NfaState<StateType::Utf8>::serialize(std::unordered_map<NfaState const*, uint32_t> const& state_ids)
    const -> std::optional<std::string>;

} // namespace log_surgeon::finite_automata