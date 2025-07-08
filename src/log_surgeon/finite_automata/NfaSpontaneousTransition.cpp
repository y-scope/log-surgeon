#include <log_surgeon/finite_automata/NfaSpontaneousTransition.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>

#include <fmt/format.h>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
auto NfaSpontaneousTransition<TypedNfaState>::serialize(
        std::unordered_map<TypedNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string> {
    if (false == state_ids.contains(m_dest_state)) {
        return std::nullopt;
    }
    auto transformed_operations
            = m_tag_ops | std::ranges::views::transform(&TagOperation::serialize);

    return fmt::format(
            "{}[{}]",
            state_ids.at(m_dest_state),
            fmt::join(transformed_operations, ",")
    );
}

template auto NfaSpontaneousTransition<NfaState<StateType::Byte>>::serialize(
        std::unordered_map<ByteNfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;

template auto NfaSpontaneousTransition<NfaState<StateType::Utf8>>::serialize(
        std::unordered_map<Utf8NfaState const*, uint32_t> const& state_ids
) const -> std::optional<std::string>;
}  // namespace log_surgeon::finite_automata
