#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_CAPTURE_QUERY_TOKEN_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_CAPTURE_QUERY_TOKEN_HPP

#include <compare>
#include <cstdint>
#include <string>
#include <utility>

namespace log_surgeon::wildcard_query_parser {
/**
 * Represents a capture in the query as a token.
 *
 * Stores a substring from the query with metadata specifying:
 * 1. The capture name.
 * 2. If the capture contains a wildcard.
 */
class CaptureQueryToken {
public:
    CaptureQueryToken(
            std::string name,
            std::string query_substring,
            bool const contains_wildcard
    )
            : m_name(std::move(name)),
              m_query_substring(std::move(query_substring)),
              m_contains_wildcard(contains_wildcard) {}

    // Must be defined if `operator<=>` is not defaulted.
    auto operator==(CaptureQueryToken const& rhs) const -> bool {
        return (*this <=> rhs) == std::strong_ordering::equal;
    }

    /**
     * Lexicographical three-way comparison operator.
     *
     * Compares member variables in the following order:
     * 1. `m_name`
     * 2. `m_query_substring`
     * 3. `m_contains_wildcard` (with `false` considered less than `true`)
     *
     * @param rhs The `CaptureQueryToken` to compare against.
     * @return The relative ordering of `this` with respect to `rhs`.
     */
    auto operator<=>(CaptureQueryToken const& rhs) const -> std::strong_ordering;

    [[nodiscard]] auto get_name() const -> std::string const& { return m_name; }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

    [[nodiscard]] auto get_contains_wildcard() const -> bool { return m_contains_wildcard; }

private:
    std::string m_name;
    std::string m_query_substring;
    bool m_contains_wildcard{false};
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_CAPTURE_QUERY_TOKEN_HPP
