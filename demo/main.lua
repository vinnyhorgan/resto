require("test")

local Cat = pesto.Object:extend()

print(pesto.ecs)

function Cat:new() self.name = "dude" end

local kitten = Cat()

print(kitten.name)

local kitten2 = Cat()

print("Kitten2: " .. kitten2.name)

print(pesto.json.encode(kitten2))

local x = 0

function pesto.update(dt)
    pesto.graphics.circle(x, 350, 25)
    x = x + dt * 500

    if x > 600 then x = 400 end
end
