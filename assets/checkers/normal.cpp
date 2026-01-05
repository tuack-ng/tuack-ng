#include "testlib.h"
#include <algorithm>
#include <string>

int main(int argc, char* argv[])
{
    registerTestlibCmd(argc, argv);

    std::string expected, output;

    // 读取标准答案文件
    while (!ans.eof()) {
        expected += ans.readLine();
        if (!ans.eof()) {
            expected += '\n';
        }
    }

    // 读取用户输出文件
    while (!ouf.eof()) {
        output += ouf.readLine();
        if (!ouf.eof()) {
            output += '\n';
        }
    }

    // 去除两个字符串末尾的空白字符（包括换行符、空格、回车、制表符）
    auto rtrim = [](std::string& s) {
        s.erase(std::find_if(s.rbegin(), s.rend(),
                    [](unsigned char ch) { return !std::isspace(ch); })
                    .base(),
            s.end());
    };

    rtrim(expected);
    rtrim(output);

    // 比较处理后的字符串
    if (expected == output) {
        quitf(_ok, "AC");
    } else {
        quitf(_wa, "Output differs from answer");
    }
}