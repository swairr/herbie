{
  "targets": [
    {
      "target_name": "herbie_winhook",
      "sources": ["index.cpp"],
      "include_dirs": ["<!@(node -p \"require('path').resolve(require('node-addon-api').include_dir).split(require('path').sep).join('/')\")"],
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