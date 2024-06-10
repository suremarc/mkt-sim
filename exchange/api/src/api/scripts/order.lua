-- Match an order (market or limit)

local asset_id, book_bid, book_offer, account_id, order_id = unpack(KEYS)
local side, size, order_type, price = unpack(ARGV)
local size = tonumber(size)
local original_size = size
local price = tonumber(price or 0)

local book_to_match
local book_to_insert
local lower, upper
local score
if side == 'bids' then
    -- Process buy order, match with asks
    book_to_match = book_offer
    book_to_insert = book_bid
    score = -price
    lower, upper = -math.huge, price
elseif side == 'offers' then
    book_to_match = book_bid
    book_to_insert = book_offer
    score = price
    lower, upper = -price, math.huge
end


local completed_order_ids = {}
local shares_filled = 0
local dollar_volume_filled = 0

local candidates = redis.call('ZRANGE', book_to_match, lower, upper, 'BYSCORE', 'WITHSCORES')
redis.log(redis.LOG_WARNING, book_to_match, lower, upper)

for i = 1, #candidates, 2 do 
    local matching_order_id, matching_order_price = candidates[i], candidates[i+1]
    local matching_size = tonumber(redis.call('HGET', matching_order_id, 'size'))
    
    if matching_size > size then
        redis.call('HINCRBY', matching_order_id, 'size', -size)
        size = 0
    else
        table.insert(completed_order_ids, matching_order_id)
        dollar_volume_filled = dollar_volume_filled + matching_order_price * matching_size
        size = size - matching_size
    end

    if size == 0 then
        break
    end
end

local completed_orders = {}
for _, order_id in ipairs(completed_order_ids) do
    local account_id = redis.call('HGET', order_id, 'account_id')
    redis.call('ZREM', account_id, order_id)
    table.insert(completed_orders, order_id)
    table.insert(completed_orders, redis.call('HGETALL', order_id))
end
if #completed_order_ids > 0 then
    redis.call('ZREM', book_to_match, unpack(completed_order_ids))
    redis.call('DEL', unpack(completed_order_ids))
end

if order_type == 'limit' and size > 0 then
    -- add remaining quantity to the order book_to_match
    redis.call('ZADD', book_to_insert, score, order_id)
    redis.call('HSET', order_id, 
        'account_id', account_id,
        'asset_id', asset_id, 
        'price', price,
        'size', size,
        'original_size', original_size,
        'dollar_volume_filled', dollar_volume_filled
    )
    redis.call('ZADD', account_id, 0, order_id)
end

return {original_size - size, dollar_volume_filled, completed_orders}
