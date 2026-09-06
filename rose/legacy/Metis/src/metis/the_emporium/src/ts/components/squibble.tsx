import React from 'react';
import {DashComponentProps} from '../props';

type Props = {
    // Insert props
} & DashComponentProps;

/**
 * Component description
 */
const squibble = (props: Props) => {
    const { id } = props;
    return (
        <div id={id}>
            <h4>You've been gnomed</h4>
        </div>
    )
}

squibble.defaultProps = {};

export default squibble;
