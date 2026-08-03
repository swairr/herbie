{
  "targets": [
    {
      "target_name": "herbie_winhook",
      "sources": ["index.cpp"],
      "include_dirs": ["<!@(node -p \"require('node-addon-api').include_dir\")"],
      "dependencies": ["<!(node -p \"require('node-addon-api').gyp\")"],
      "defines": ["NAPI_DISABLE_CPP_EXCEPTIONS"],
      "conditions": [
        [
          "OS=='win'",
          {
            "libraries": ["user32.lib", "psapi.lib", "kernel32.lib"]
          }
        ]
      ]
    }
  ]
}