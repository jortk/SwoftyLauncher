// Host unit test for the hardware settings probe logic (hw_settings_logic.hpp). run_events_test.sh builds and runs it.
#include <cstdio>
#include <string>

#include "../src/core/hw_settings_logic.hpp"

static int fails;

static void expect(bool ok, const std::string &what)
{
    printf("%s: %s\n", ok ? "ok  " : "FAIL", what.c_str());
    fails += !ok;
}

int main()
{
    namespace hw = hw_settings;
    expect(!hw::cloud_only(0x20), "an archive-only file is local");
    expect(hw::cloud_only(hw::ATTR_RECALL_ON_DATA_ACCESS), "recall on data access is cloud-only");
    expect(hw::cloud_only(hw::ATTR_RECALL_ON_OPEN), "recall on open is cloud-only");
    expect(hw::cloud_only(hw::ATTR_OFFLINE), "offline is cloud-only");
    expect(hw::cloud_flags(0x20 | hw::ATTR_RECALL_ON_DATA_ACCESS | hw::ATTR_OFFLINE) == "recall_on_data_access+offline",
           "flags are named in a fixed order");
    expect(hw::under_onedrive(L"C:\\Users\\malte\\OneDrive\\Dokumente\\My Games\\F1 25"), "OneDrive in the path");
    expect(hw::under_onedrive(L"C:\\Users\\id\\onedrive - Org\\Documents"), "a business OneDrive folder, any case");
    expect(!hw::under_onedrive(L"C:\\Users\\vikto\\Documents\\My Games\\F1 25"), "a local Documents folder");
    expect(!hw::under_onedrive(L"C:\\Users\\id\\Documents\\MyOneDrive"), "OneDrive inside a name is not a OneDrive folder");

    hw::Probe p;
    p.path = "C:\\Users\\malte\\OneDrive\\Dokumente\\My Games\\F1 25\\hardwaresettings\\hardware_settings_config.xml";
    p.onedrive = true;
    p.exists = true;
    p.size = 5120;
    p.attrs = 0x20 | hw::ATTR_RECALL_ON_DATA_ACCESS;
    std::string cloud = hw::line(p);
    expect(cloud.find("onedrive=yes") != std::string::npos && cloud.find("cloud-only=yes (recall_on_data_access)") != std::string::npos &&
               cloud.find("size=5120") != std::string::npos && cloud.find("read test: skipped") != std::string::npos,
           "a cloud-only file is named and not read: " + cloud);
    p.attrs = 0x20;
    p.read_tried = p.read_ok = true;
    p.read_bytes = 4096;
    expect(hw::line(p).find("cloud-only=no read test: ok (4096 bytes)") != std::string::npos, "a local file reads");
    p.read_ok = false;
    p.read_error = 389;
    expect(hw::line(p).find("read test: FAILED (Windows error 389)") != std::string::npos, "a failed read carries the error");
    hw::Probe none;
    none.path = "C:\\x.xml";
    expect(hw::line(none) == "[hwsettings] C:\\x.xml: onedrive=no exists=no", "a missing file is one short line");
    return fails ? 1 : 0;
}
