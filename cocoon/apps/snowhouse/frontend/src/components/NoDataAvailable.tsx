import { cn } from '@/lib/utils';
import { Inbox } from "lucide-react";

interface NoDataAvailableProps {
    message?: string;
    description?: string;
    className?: string;
    icon?: React.ReactNode;
}

export default function NoDataAvailable(
    {
        message = "No data available",
        description,
        className,
        icon = <Inbox className="w-10 h-10 text-muted-foreground" strokeWidth={1.5} />
    }: NoDataAvailableProps
) {

    return (
        <div className={cn("flex flex-col items-center justify-center p-8 text-center", className)}>
            <div className="rounded-full p-3 mb-2 bg-muted">
                {icon}
            </div>
            <p className="text-[20px] font-semibold">{message}</p>
            {description && (
                <p className="text-sm text-muted-foreground mt-1">{description}</p>
            )}
        </div>
    );
}
