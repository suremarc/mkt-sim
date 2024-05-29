-- Match an order (market or limit)

local asset_id, book_ask, book_bid, account_id, order_id = unpack(KEYS)
local size, side, order_type, price = unpack(ARGV)
local size = tonumber(size)
local price = tonumber(price or 0)

local book_to_match
local book_to_insert
local lower, upper
local score
if side == 'buy' then
    -- Process buy order, match with asks
    book_to_match = book_ask
    book_to_insert = book_bid
    score = -price
    lower, upper = price, math.huge
elseif side == 'sell' then
    book_to_match = book_bid
    book_to_insert = book_ask
    score = price
    lower, upper = -math.huge, -price
end

local candidates = redis.call('ZRANGE', book_to_match, lower, upper, 'BYSCORE')
redis.log(redis.LOG_WARNING, book_to_match, lower, upper)

for _, matching_order_id in ipairs(candidates) do 
    local matching_size = tonumber(redis.call('HGET', matching_order_id, 'size'))
    
    if matching_size > size then
        redis.call('HINCRBY', matching_order_id, 'size', -size)
        size = 0
    else
        redis.call('ZREM', book_to_match, matching_order_id)
        local other_order_account = redis.call('HGET', matching_order_id, 'account_id')
        redis.call('ZREM', other_order_account, matching_order_id)
        redis.call('DEL', matching_order_id)
        size = size - matching_size
    end

    if size == 0 then
        break
    end
end

if order_type == 'limit' and size > 0 then
    -- add remaining quantity to the order book_to_match
    redis.call('ZADD', book_to_insert, score, order_id)
    redis.call('HSET', order_id, 
        'account_id', account_id, 
        'asset_id', asset_id, 
        'price', price,
        'size', size,
        'side', side
    )
    redis.call('ZADD', account_id, 0, order_id)
end

return size -- Return the remaining unmatched size
