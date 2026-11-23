local marker = "★ "

return {
    mark = function(_, args)
        local title = args.tab.title
        if string.sub(title, 1, #marker) == marker then
            return { action = "rename", name = "" }
        end
        return { action = "rename", name = marker .. title }
    end,
    group = function(_, args)
        local group = args.groups[1]
        if not group then
            return nil
        end
        if args.tab.group_id == group.id then
            return { action = "assign_group" }
        end
        return { action = "assign_group", group_id = group.id }
    end,
    new_peer = function()
        return { action = "shell_picker" }
    end,
}
