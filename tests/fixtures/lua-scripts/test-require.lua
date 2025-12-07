-- Template for testing package loading
-- NOTE: In tests, replace this with ctx.lua_path_setup() output
-- This is just a reference template
package.path = './lua_modules/?.lua;./lua_modules/?/init.lua;' .. package.path
package.cpath = './lua_modules/?.so;./lua_modules/?.dylib;./lua_modules/?.dll;' .. package.cpath

local function test_require(module_name)
    local ok, result = pcall(require, module_name)
    if not ok then
        print("ERROR: Failed to require '" .. module_name .. "'")
        print("Error: " .. tostring(result))
        os.exit(1)
    end
    return result
end

return test_require

