#ifndef LOG_SURGEON_CONSTANTS_HPP
#define LOG_SURGEON_CONSTANTS_HPP

#include <cstdint>

namespace log_surgeon {
constexpr uint32_t cUnicodeMax = 0x10'FFFF;
constexpr uint32_t cSizeOfByte = 256;
constexpr uint32_t cSizeOfAllChildren = 10'000;
constexpr uint32_t cNullSymbol = 10'000'000;

enum class ErrorCode : std::uint8_t {
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

enum class SymbolID : std::uint8_t {
    TokenEndID,
    TokenUncaughtStringID,
    TokenIntId,
    TokenFloatId,
    TokenHexId,
    TokenFirstTimestampId,
    TokenNewlineTimestampId,
    TokenNewlineId
};

constexpr auto cTokenEnd = "$end";
constexpr auto cTokenUncaughtString = "$UncaughtString";
constexpr auto cTokenInt = "int";
constexpr auto cTokenFloat = "float";
constexpr auto cTokenHex = "hex";
constexpr auto cTokenFirstTimestamp = "firstTimestamp";
constexpr auto cTokenNewlineTimestamp = "newLineTimestamp";
constexpr auto cTokenNewline = "newLine";
constexpr uint32_t cStaticByteBuffSize = 48'000;

namespace utf8 {
// 0xC0, 0xC1, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE,
// 0xFF are invalid UTF-8 code units
static constexpr unsigned char cCharEOF = 0xFF;
static constexpr unsigned char cCharErr = 0xFE;
static constexpr unsigned char cCharStartOfFile = 0xFD;
}  // namespace utf8
}  // namespace log_surgeon

#endif  // LOG_SURGEON_CONSTANTS_HPP
