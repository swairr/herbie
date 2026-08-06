// herbie-winhook: emits Windows foreground window change + title change events.
// Business logic lives in TypeScript (src/main/segments.ts); this module only reports
// { type, hwnd, processName, title } events. Windows-only.

#include <napi.h>
#include <windows.h>
#include <psapi.h>
#include <string>
#include <thread>
#include <atomic>
#include <unordered_map>

using Napi::Env;
using Napi::Object;
using Napi::Function;
using Napi::String;
using Napi::Number;
using Napi::ThreadSafeFunction;

namespace {

HWINEVENTHOOK g_fgHook = nullptr;
HWINEVENTHOOK g_nameHook = nullptr;
ThreadSafeFunction g_tsf;
std::thread g_thread;
std::atomic<bool> g_running{false};
DWORD g_threadId = 0;

static std::string WideToUtf8(const std::wstring& w) {
  if (w.empty()) return std::string();
  int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), nullptr, 0, nullptr, nullptr);
  std::string s(n, 0);
  WideCharToMultiByte(CP_UTF8, 0, w.c_str(), (int)w.size(), &s[0], n, nullptr, nullptr);
  return s;
}

static std::wstring GetProcessBaseName(HWND hwnd) {
  std::wstring empty;
  DWORD pid = 0;
  if (!GetWindowThreadProcessId(hwnd, &pid) || pid == 0) return empty;
  // The image name is invariant for a pid's lifetime; cache it to avoid OpenProcess +
  // QueryFullProcessImageName syscalls on every (frequent) namechange event.
  static std::unordered_map<DWORD, std::wstring> cache;
  auto it = cache.find(pid);
  if (it != cache.end()) return it->second;
  HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
  if (!h) {
    // Elevated process: cannot query image name. Fall back to empty (no failure).
    cache[pid] = empty;
    return empty;
  }
  wchar_t buf[MAX_PATH] = {0};
  DWORD size = MAX_PATH;
  BOOL ok = QueryFullProcessImageNameW(h, 0, buf, &size);
  CloseHandle(h);
  if (!ok || size == 0) {
    cache[pid] = empty;
    return empty;
  }
  std::wstring full(buf, size);
  size_t slash = full.find_last_of(L"\\/");
  std::wstring base = slash == std::wstring::npos ? full : full.substr(slash + 1);
  cache[pid] = base;
  return base;
}

static std::string GetWindowTitle(HWND hwnd) {
  wchar_t buf[512] = {0};
  int n = GetWindowTextW(hwnd, buf, 512);
  if (n <= 0) return std::string();
  return WideToUtf8(std::wstring(buf, n));
}

static void EmitEvent(const char* type, HWND hwnd) {
  if (!g_running) return;
  std::string proc = WideToUtf8(GetProcessBaseName(hwnd));
  std::string title = GetWindowTitle(hwnd);
  // Dedup: chatty foreground windows mutate their title repeatedly (often identically).
  // Skip emission when nothing changed since the last emitted event to avoid an event
  // storm across the thread boundary plus a flood of segment close/open writes.
  static std::string lastType;
  static std::string lastProc;
  static std::string lastTitle;
  std::string typeStr(type);
  if (typeStr == lastType && proc == lastProc && title == lastTitle) {
    return;
  }
  lastType = typeStr;
  lastProc = proc;
  lastTitle = title;
  uintptr_t hwndVal = reinterpret_cast<uintptr_t>(hwnd);
  g_tsf.NonBlockingCall([typeStr, hwndVal, proc, title](Env env, Function js) {
    try {
      Object event = Object::New(env);
      event.Set("type", String::New(env, typeStr));
      event.Set("hwnd", Number::New(env, (double)hwndVal));
      event.Set("processName", String::New(env, proc));
      event.Set("title", String::New(env, title));
      js.Call({event});
    } catch (const Napi::Error& error) {
      // Never allow a JavaScript exception to escape a ThreadSafeFunction callback.
      // Node otherwise reports DEP0168 and the exception may terminate the process.
      error.ThrowAsJavaScriptException();
    } catch (...) {
      Napi::Error::New(env, "herbie-winhook callback failed").ThrowAsJavaScriptException();
    }
  });
}

static void CALLBACK WinEventProc(HWINEVENTHOOK /*hook*/, DWORD event, HWND hwnd,
                                  LONG /*idObject*/, LONG /*idChild*/,
                                  DWORD /*dwEventThread*/, DWORD /*dwmsEventTime*/) {
  // Only report windows (idObject == OBJID_WINDOW == 0). Filter spurious notifications.
  if (event == EVENT_OBJECT_NAMECHANGE) {
    HWND fg = GetForegroundWindow();
    if (hwnd != fg) return; // enforce native-side "current foreground only" filter
  }
  const char* type = (event == EVENT_SYSTEM_FOREGROUND) ? "foreground" : "namechange";
  EmitEvent(type, hwnd);
}

static void PumpThread() {
  g_threadId = GetCurrentThreadId();
  g_fgHook = SetWinEventHook(
    EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
    nullptr, WinEventProc, 0, 0,
    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
  g_nameHook = SetWinEventHook(
    EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE,
    nullptr, WinEventProc, 0, 0,
    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

  MSG msg;
  while (g_running.load()) {
    BOOL r = GetMessage(&msg, nullptr, 0, 0);
    if (r <= 0) break;
    TranslateMessage(&msg);
    DispatchMessage(&msg);
  }
  if (g_fgHook) { UnhookWinEvent(g_fgHook); g_fgHook = nullptr; }
  if (g_nameHook) { UnhookWinEvent(g_nameHook); g_nameHook = nullptr; }
}

} // namespace

static void Start(const Napi::CallbackInfo& info) {
  Env env = info.Env();
  if (g_running.load()) return;
  if (info.Length() < 1 || !info[0].IsFunction()) {
    Napi::TypeError::New(env, "start(cb) expects a callback").ThrowAsJavaScriptException();
    return;
  }
  Function cb = info[0].As<Function>();
  g_tsf = ThreadSafeFunction::New(env, cb, "herbie-winhook", 0, 1);
  g_running.store(true);
  g_thread = std::thread(PumpThread);
}

static void Stop(const Napi::CallbackInfo& info) {
  (void)info;
  if (!g_running.load()) return;
  g_running.store(false);
  if (g_threadId != 0) {
    PostThreadMessage(g_threadId, WM_QUIT, 0, 0);
  }
  if (g_thread.joinable()) g_thread.join();
  g_tsf.Release();
  g_threadId = 0;
}

static Object Init(Env env, Object exports) {
  exports.Set(String::New(env, "start"), Function::New(env, Start));
  exports.Set(String::New(env, "stop"), Function::New(env, Stop));
  return exports;
}

NODE_API_MODULE(herbie_winhook, Init)
