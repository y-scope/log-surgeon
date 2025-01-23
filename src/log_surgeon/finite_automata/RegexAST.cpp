#include "RegexAST.hpp"

#include <fmt/core.h>
#include <fmt/ranges.h>
#include <fmt/xchar.h>

namespace log_surgeon::finite_automata {
template<typename TypedNfaState>
auto RegexAST<TypeNfaState>::serialize_negative_tags() const -> std::u32string {
    std::u32string negative_tags_serialized;
    if (m_negative_tags.empty()) {
        return U"";
    }

    auto const transformed_tags = m_negative_tags
            | std::ranges::views::transform([](Tag const* tag) { return tag->get_name(); });
    for (auto const& tag_name : transformed_tags) {
        if (false == negative_tags_serialized.empty()) {
            negative_tags_serialized += U", ";  // Add separator
        }
        negative_tags_serialized += tag_name;
    }
    return fmt::format(U"[^{}]", negative_tags_serialized);
}

template <typename TypedNfaState>
auto RegexASTEmpty<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(U"{}", RegexAST<TypedNfaState>::serialize_negative_tags());
}

template <typename TypedNfaState>
auto RegexASTLiteral<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}",
            static_cast<char32_t>(m_character),
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

template <typename TypedNfaState>
auto  RegexASTInteger<TypedNfaState>::serialize() const -> std::u32string {
    auto const digits_string = fmt::format("{}", fmt::join(m_digits, ""));
    return fmt::format(
            U"{}{}",
            std::u32string(digits_string.begin(), digits_string.end()),
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

template <typename TypedNfaState>
auto RegexASTOr<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"({})|({}){}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

template <typename TypedNfaState>
auto RegexASTCat<TypedNfaState>::serialize() const -> std::u32string {
    return fmt::format(
            U"{}{}{}",
            nullptr != m_left ? m_left->serialize() : U"null",
            nullptr != m_right ? m_right->serialize() : U"null",
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

template <typename TypedNfaState>
auto RegexASTMultiplication<TypedNfaState>::serialize() const -> std::u32string {
    auto const min_string = std::to_string(m_min);
    auto const max_string = std::to_string(m_max);

    return fmt::format(
            U"({}){{{},{}}}{}",
            nullptr != m_operand ? m_operand->serialize() : U"null",
            std::u32string(min_string.begin(), min_string.end()),
            is_infinite() ? U"inf" : std::u32string(max_string.begin(), max_string.end()),
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

template <typename TypedNfaState>
auto RegexASTCapture<TypedNfaState>::serialize() const -> std::u32string {
    auto const tag_name_u32 = std::u32string(m_tag->get_name().cbegin(), m_tag->get_name().cend());
    return fmt::format(
            U"({})<{}>{}",
            m_group_regex_ast->serialize(),
            tag_name_u32,
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

template <typename TypedNfaState>
auto RegexASTGroup<TypedNfaState>::serialize() const -> std::u32string {
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
            RegexAST<TypedNfaState>::serialize_negative_tags()
    );
}

// Explicit template instantiations
template auto RegexAST<ByteNfaState>::serialize_negative_tags() const -> std::u32string;
template auto RegexAST<Utf8NfaState>::serialize_negative_tags() const -> std::u32string;
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
} // namespace log_surgeon::finite_automata
