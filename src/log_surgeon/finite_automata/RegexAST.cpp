#include "RegexAST.hpp"

#include <cstdint>
#include <ranges>
#include <string>
#include <utility>

#include <log_surgeon/finite_automata/Capture.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>

#include <fmt/core.h>
#include <fmt/format.h>
#include <fmt/ranges.h>
#include <fmt/xchar.h>

namespace log_surgeon::finite_automata {
template <typename TypedNfaState>
auto RegexAST<TypedNfaState>::serialize_negative_captures() const -> std::u32string {
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

template <typename TypedNfaState>
[[nodiscard]] auto RegexASTEmpty<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(U"{}", RegexAST<TypedNfaState>::serialize_negative_captures());
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
[[nodiscard]] auto RegexASTInteger<TypedNfaState>::serialize() const -> std::u32string {
    auto const digits_string = fmt::format("{}", fmt::join(m_digits, ""));
    return fmt::format(
            U"{}{}",
            std::u32string(digits_string.begin(), digits_string.end()),
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
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
[[nodiscard]] auto RegexASTCat<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}{}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<TypedNfaState>::serialize_negative_captures()
    );
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

template auto RegexAST<ByteNfaState>::serialize_negative_captures() const -> std::u32string;
template auto RegexAST<Utf8NfaState>::serialize_negative_captures() const -> std::u32string;

template auto RegexASTEmpty<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTEmpty<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTLiteral<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTLiteral<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTInteger<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTInteger<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTOr<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTOr<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTCat<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTCat<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTMultiplication<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTMultiplication<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTCapture<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTCapture<Utf8NfaState>::serialize() const -> std::u32string;

template auto RegexASTGroup<ByteNfaState>::serialize() const -> std::u32string;
template auto RegexASTGroup<Utf8NfaState>::serialize() const -> std::u32string;
}  // namespace log_surgeon::finite_automata
