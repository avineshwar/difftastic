
map3 func ma mb mc =
    case ma of
        Nothing ->
            Nothing

        Just a ->
            case mb of
                Nothing ->
                    Nothing

                Just b ->
                    b


map4 func ma mb mc md =
    case ma of
        Nothing ->
            Nothing

        Just a ->
            a
