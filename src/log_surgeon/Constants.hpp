#ifndef LOG_SURGEON_CONSTANTS_HPP
#define LOG_SURGEON_CONSTANTS_HPP

#include <cstdint>
#include <utility>

namespace log_surgeon {
constexpr uint32_t cUnicodeMax = 0x10'FFFF;
constexpr uint32_t cSizeOfUnicode = cUnicodeMax + 1;
constexpr uint32_t cSizeOfByte = 256;
constexpr uint32_t cSizeOfAllChildren = 10'000;
constexpr uint32_t cNullSymbol = 10'000'000;

enum class ErrorCode {
    Success,
    BufferOutOfBounds,
    LogFullyConsumed,
    BadParam,
    Errno,
    EndOfFile,
    FileNotFound,
    NotInit,
    Truncated,
};

/**
 * Enum for token IDs.
 * NOTE: we use `uint32_t` as the underlying type to ensure consistency with token identifier types.
 */
enum class SymbolId : uint32_t {
    TokenEnd,
    TokenUncaughtString,
    TokenInt,
    TokenFloat,
    TokenHex,
    TokenFirstTimestamp,
    TokenNewlineTimestamp,
    TokenNewline
};

constexpr char cTokenEnd[] = "$end";
constexpr char cTokenUncaughtString[] = "$UncaughtString";
constexpr char cTokenInt[] = "int";
constexpr char cTokenFloat[] = "float";
constexpr char cTokenHex[] = "hex";
constexpr char cTokenFirstTimestamp[] = "firstTimestamp";
constexpr char cTokenNewlineTimestamp[] = "newLineTimestamp";
constexpr char cTokenNewline[] = "newLine";
constexpr uint32_t cStaticByteBuffSize = 48'000;

namespace utf8 {
// 0xC0, 0xC1, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE,
// 0xFF are invalid UTF-8 code units
static unsigned char const cCharEOF = 0xFF;
static unsigned char const cCharErr = 0xFE;
static unsigned char const cCharStartOfFile = 0xFD;
}  // namespace utf8
}  // namespace log_surgeon

#endif  // LOG_SURGEON_CONSTANTS_HPP
